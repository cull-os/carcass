use std::{
   collections,
   io,
   iter,
   net,
   path,
   sync::{
      self,
      Arc,
      atomic,
   },
   time,
};

use dup::Dupe;
use hickory_server::{
   net::{
      self as hickory_net,
      runtime as hickory_runtime,
   },
   proto::{
      rr::{
         self,
         rdata,
      },
      serialize::txt as hickory_txt,
   },
   resolver::config as resolver_config,
   server as hserver,
   store::{
      forwarder,
      in_memory,
   },
   zone_handler,
};
use libp2p as p2p;
use tokio::{
   fs,
   net as tokio_net,
   sync::RwLock,
   task,
};

use crate::{
   address,
   config,
};

macro_rules! secs {
   ($duration:expr) => {
      secs!($duration, u32)
   };
   ($duration:expr, $type:ty) => {
      <$type>::try_from($duration.as_secs()).expect(concat!(
         stringify!($duration),
         " must fit in ",
         stringify!($type),
      ))
   };
}

pub const PORT: u16 = 67;

#[derive(Debug, thiserror::Error)]
pub enum Error {
   #[error("failed to bind '{address}'")]
   Bind {
      address: net::SocketAddr,
      #[source]
      source:  io::Error,
   },

   #[error("failed to read zone file '{path}'")]
   ReadZone {
      path:   path::PathBuf,
      #[source]
      source: io::Error,
   },

   #[error("failed to parse zone")]
   ParseZone(#[source] hickory_txt::ParseError),

   #[error("zone $ORIGIN '{got}' does not match expected '{expected}'")]
   ZoneMismatch {
      expected: rr::Name,
      got:      rr::Name,
   },

   #[error("dns server task failed")]
   Server(#[source] hickory_net::NetError),
}

static APEX: sync::LazyLock<rr::Name> = sync::LazyLock::new(|| {
   rr::Name::from_labels(["67", "internal"]).expect("apex labels must form a valid dns name")
});

static REVERSE_APEX: sync::LazyLock<rr::Name> =
   sync::LazyLock::new(|| reverse_zone_of(address::Prefix::LOCAL));

const TTL: time::Duration = time::Duration::from_hours(1);

fn zone_of(peer_id: p2p::PeerId) -> rr::Name {
   APEX
      .prepend_label(multibase::encode(multibase::Base::Base32Lower, peer_id.to_bytes()).as_str())
      .expect("base32lower peerID must fit in a dns label")
}

fn reverse_zone_of(prefix: address::Prefix) -> rr::Name {
   const NIBBLES_PER_BYTE: usize = 2;
   const SUFFIX_LABELS: usize = 2;

   rr::Name::from(net::Ipv6Addr::from(prefix))
      .trim_to(address::HOST_PREFIX_RANGE.end * NIBBLES_PER_BYTE + SUFFIX_LABELS)
}

#[bon::builder(finish_fn(name = "records"))]
fn zone_handler_of(
   #[builder(start_fn)] zone: &rr::Name,
   #[builder(finish_fn)] records: impl Iterator<Item = rr::Record>,
   serial: u32,
   glue: Option<net::Ipv6Addr>,
) -> in_memory::InMemoryZoneHandler<hickory_runtime::TokioRuntimeProvider> {
   let soa = (
      rr::RrKey::new(rr::LowerName::from(zone), rr::RecordType::SOA),
      rr::RecordSet::from(rr::Record::from_rdata(
         zone.clone(),
         secs!(TTL),
         rr::RData::SOA(rdata::SOA::new(
            // Use the zone itself for the primary authority and responsible party.
            zone.clone(),
            zone.clone(),
            serial,
            secs!(time::Duration::from_hours(2), i32),
            secs!(time::Duration::from_hours(1), i32),
            secs!(time::Duration::from_weeks(2), i32),
            secs!(TTL),
         )),
      )),
   );

   let mut handler = in_memory::InMemoryZoneHandler::<hickory_runtime::TokioRuntimeProvider>::new(
      zone.clone(),
      iter::once(soa).collect(),
      zone_handler::ZoneType::Primary,
      zone_handler::AxfrPolicy::Deny,
   )
   .expect("zone construction with valid soa cannot fail");

   if let Some(address) = glue {
      handler.upsert_mut(
         rr::Record::from_rdata(
            zone.clone(),
            secs!(TTL),
            rr::RData::NS(rdata::NS(zone.clone())),
         ),
         serial,
      );
      handler.upsert_mut(
         rr::Record::from_rdata(
            zone.clone(),
            secs!(TTL),
            rr::RData::AAAA(rdata::AAAA(address)),
         ),
         serial,
      );
   }

   for record in records {
      handler.upsert_mut(record, serial);
   }

   handler
}

#[bon::builder(finish_fn(name = "prefix"))]
fn forwarder_of(
   #[builder(start_fn)] zone: rr::Name,
   #[builder(finish_fn)] prefix: address::Prefix,
) -> forwarder::ForwardZoneHandler<hickory_runtime::TokioRuntimeProvider> {
   let upstream = resolver_config::NameServerConfig::new(
      net::IpAddr::from(prefix.host_addr()),
      true, // trust_negative_responses
      vec![{
         let mut connection = resolver_config::ConnectionConfig::udp();

         connection.port = PORT;

         connection
      }],
   );

   forwarder::ForwardZoneHandler::builder_tokio(forwarder::ForwardConfig {
      name_servers: vec![upstream],
      options:      None,
   })
   .with_origin(zone)
   .build()
   .expect("forwarder build cannot fail without tls or dnssec features")
}

fn next_serial() -> u32 {
   static SERIAL: sync::LazyLock<atomic::AtomicU32> = sync::LazyLock::new(|| {
      #[expect(
         clippy::cast_possible_truncation,
         reason = "u32 wraps in year 2106, but RFC 1982 still holds even then"
      )]
      atomic::AtomicU32::new(
         time::SystemTime::now()
            .duration_since(time::UNIX_EPOCH)
            .expect("system clock must be after unix epoch")
            .as_secs() as u32,
      )
   });

   SERIAL.fetch_add(1, atomic::Ordering::Relaxed)
}

async fn bind(
   server: &mut hserver::Server<impl hserver::RequestHandler>,
   address: net::SocketAddr,
) -> Result<(), Error> {
   const TCP_TIMEOUT: time::Duration = time::Duration::from_secs(5);
   const TCP_RESPONSE_QUEUE: usize = 32;

   server.register_socket(
      tokio_net::UdpSocket::bind(&address)
         .await
         .map_err(|source| Error::Bind { address, source })?,
   );

   server.register_listener(
      tokio_net::TcpListener::bind(&address)
         .await
         .map_err(|source| Error::Bind { address, source })?,
      TCP_TIMEOUT,
      TCP_RESPONSE_QUEUE,
   );

   Ok(())
}

#[derive(Clone, Dupe)]
pub struct Local {
   catalog: Arc<RwLock<zone_handler::Catalog>>,
}

#[async_trait::async_trait]
impl hserver::RequestHandler for Local {
   async fn handle_request<R: hserver::ResponseHandler, T: hickory_runtime::Time>(
      &self,
      request: &hserver::Request,
      response_handle: R,
   ) -> hserver::ResponseInfo {
      self
         .catalog
         .read()
         .await
         .handle_request::<R, T>(request, response_handle)
         .await
   }
}

impl Local {
   #[must_use]
   pub fn new() -> Self {
      Self {
         catalog: Arc::new(RwLock::new(zone_handler::Catalog::new())),
      }
   }

   pub async fn listen(
      &self,
      join_set: &mut task::JoinSet<Result<(), Error>>,
   ) -> Result<(), Error> {
      let mut server = hserver::Server::new(self.dupe());

      bind(
         &mut server,
         net::SocketAddr::new(net::IpAddr::from(address::Prefix::LOCAL.host_addr()), PORT),
      )
      .await?;

      join_set.spawn(async move { server.block_until_done().await.map_err(Error::Server) });

      Ok(())
   }

   pub async fn reload(&self, map: &address::Map) {
      let serial = next_serial();

      *self.catalog.write().await = {
         let mut catalog = zone_handler::Catalog::new();

         catalog.upsert(rr::LowerName::from(&*APEX), vec![Arc::new(
            zone_handler_of(&APEX)
               .serial(serial)
               .glue(address::Prefix::LOCAL.host_addr())
               .records(iter::empty()),
         )]);

         catalog.upsert(rr::LowerName::from(&*REVERSE_APEX), vec![Arc::new(
            zone_handler_of(&REVERSE_APEX)
               .serial(serial)
               .records(iter::once(rr::Record::from_rdata(
                  rr::Name::from(net::IpAddr::from(address::Prefix::LOCAL.host_addr())),
                  secs!(TTL),
                  rr::RData::PTR(rdata::PTR(APEX.clone())),
               ))),
         )]);

         for (peer_id, prefix) in map.iter() {
            let forward_zone = zone_of(peer_id);
            catalog.upsert(rr::LowerName::from(forward_zone.clone()), vec![Arc::new(
               forwarder_of(forward_zone).prefix(prefix),
            )]);

            let reverse_zone = reverse_zone_of(prefix);
            catalog.upsert(rr::LowerName::from(reverse_zone.clone()), vec![Arc::new(
               forwarder_of(reverse_zone).prefix(prefix),
            )]);
         }

         catalog
      };
   }
}

#[derive(Clone, Dupe)]
pub struct Host {
   catalog: Arc<RwLock<zone_handler::Catalog>>,

   peer_id: p2p::PeerId,
   prefix:  address::Prefix,
}

#[async_trait::async_trait]
impl hserver::RequestHandler for Host {
   async fn handle_request<R: hserver::ResponseHandler, T: hickory_runtime::Time>(
      &self,
      request: &hserver::Request,
      response_handle: R,
   ) -> hserver::ResponseInfo {
      self
         .catalog
         .read()
         .await
         .handle_request::<R, T>(request, response_handle)
         .await
   }
}

impl Host {
   #[must_use]
   pub fn new(peer_id: p2p::PeerId, map: &address::Map) -> Self {
      Self {
         catalog: Arc::new(RwLock::new(zone_handler::Catalog::new())),
         peer_id,
         prefix: map
            .prefix_of(&peer_id)
            .expect("peer_id must be mapped before constructing host"),
      }
   }

   pub async fn listen(
      &self,
      join_set: &mut task::JoinSet<Result<(), Error>>,
   ) -> Result<(), Error> {
      let mut server = hserver::Server::new(self.dupe());

      bind(
         &mut server,
         net::SocketAddr::new(net::IpAddr::from(self.prefix.host_addr()), PORT),
      )
      .await?;

      join_set.spawn(async move { server.block_until_done().await.map_err(Error::Server) });

      Ok(())
   }

   pub async fn reload(&self, source: Option<&config::FileOrInline>) -> Result<(), Error> {
      let serial = next_serial();

      let zone = zone_of(self.peer_id);
      let reverse_zone = reverse_zone_of(self.prefix);

      let records = match source {
         None => None,
         Some(source) => {
            let (text, path) = match *source {
               config::FileOrInline::Inline(ref text) => (text.clone(), None),
               config::FileOrInline::File(ref path) => {
                  (
                     fs::read_to_string(path).await.map_err(|source| {
                        Error::ReadZone {
                           path: path.clone(),
                           source,
                        }
                     })?,
                     Some(path.clone()),
                  )
               },
            };

            let (zone_got, records) = hickory_txt::Parser::new(text, path, Some(zone.clone()))
               .parse()
               .map_err(Error::ParseZone)?;

            if zone_got != zone {
               return Err(Error::ZoneMismatch {
                  expected: zone,
                  got:      zone_got,
               });
            }

            Some(records)
         },
      }
      .into_iter()
      .flat_map(collections::BTreeMap::into_values)
      .flatten()
      .collect::<Vec<_>>();

      let reverse_records = iter::once(rr::Record::from_rdata(
         rr::Name::from(net::IpAddr::from(self.prefix.host_addr())),
         secs!(TTL),
         rr::RData::PTR(rdata::PTR(zone.clone())),
      ))
      .chain(records.iter().filter_map(|record| {
         record.data.ip_addr().map(|addr| {
            rr::Record::from_rdata(
               rr::Name::from(addr),
               record.ttl,
               rr::RData::PTR(rdata::PTR(record.name.clone())),
            )
         })
      }));

      *self.catalog.write().await = {
         let mut catalog = zone_handler::Catalog::new();

         // Reverse first as it borrows `records`.
         catalog.upsert(rr::LowerName::from(&reverse_zone), vec![Arc::new(
            zone_handler_of(&reverse_zone)
               .serial(serial)
               .records(reverse_records),
         )]);

         catalog.upsert(rr::LowerName::from(&zone), vec![Arc::new(
            zone_handler_of(&zone)
               .serial(serial)
               .glue(self.prefix.host_addr())
               .records(records.into_iter()),
         )]);

         catalog
      };

      Ok(())
   }
}
