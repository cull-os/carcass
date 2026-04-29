use std::{
   io,
   net,
   time,
};

use hickory_server::{
   net::{
      self as hickory_net,
      runtime as hickory_runtime,
   },
   proto::{
      op,
      rr::{
         self,
         rdata,
      },
   },
   server as hserver,
   zone_handler,
};
use libp2p as p2p;
use tokio::{
   net as tokio_net,
   sync::{
      mpsc,
      oneshot,
   },
   task,
};

use crate::address;

pub enum Query {
   AddressForPeerId {
      peer_id: p2p::PeerId,
      sender:  oneshot::Sender<Option<net::Ipv6Addr>>,
   },
   PeerIdForAddress {
      address: net::Ipv6Addr,
      sender:  oneshot::Sender<Option<p2p::PeerId>>,
   },
}

struct Handler {
   queries: mpsc::UnboundedSender<Query>,
}

impl Handler {
   async fn address_for(&self, name: &rr::Name) -> Option<net::Ipv6Addr> {
      let mut labels = name.iter();

      let peer_id = match (labels.next(), labels.next(), labels.next(), labels.next()) {
         (Some(peer), Some(s67), Some(internal), None)
            if s67.eq_ignore_ascii_case(b"67") && internal.eq_ignore_ascii_case(b"internal") =>
         {
            let (_, bytes) = multibase::decode(str::from_utf8(peer).ok()?).ok()?;
            p2p::PeerId::from_bytes(&bytes).ok()?
         },
         _ => return None,
      };

      let (sender, receiver) = oneshot::channel();
      self
         .queries
         .send(Query::AddressForPeerId { peer_id, sender })
         .expect("receiver must stay alive");

      receiver.await.expect("sender must not be dropped")
   }

   async fn peer_id_for(&self, name: &rr::Name) -> Option<p2p::PeerId> {
      let net::IpAddr::V6(address) = name.parse_arpa_name().ok()?.addr() else {
         return None;
      };

      let (sender, receiver) = oneshot::channel();
      self
         .queries
         .send(Query::PeerIdForAddress { address, sender })
         .expect("receiver must stay alive");

      receiver.await.expect("sender must not be dropped")
   }
}

#[async_trait::async_trait]
impl hserver::RequestHandler for Handler {
   async fn handle_request<R: hserver::ResponseHandler, T: hickory_runtime::Time>(
      &self,
      request: &hserver::Request,
      mut response_handle: R,
   ) -> hserver::ResponseInfo {
      let mut metadata = op::Metadata::response_from_request(&request.metadata);
      metadata.authoritative = true;

      let mut records = Vec::new();
      for query in request.queries.queries() {
         let name = query.name();

         let rdata = match query.query_type() {
            rr::RecordType::AAAA => {
               let Some(address) = self.address_for(name).await else {
                  continue;
               };

               rr::RData::AAAA(rdata::AAAA(address))
            },
            rr::RecordType::PTR => {
               let Some(peer_id) = self.peer_id_for(name).await else {
                  continue;
               };

               rr::RData::PTR(rdata::PTR(
                  rr::Name::from_labels([
                     multibase::encode(multibase::Base::Base32Lower, peer_id.to_bytes()).as_str(),
                     "internal",
                  ])
                  .expect("multibase base32lower label must fit in dns label"),
               ))
            },
            _ => continue,
         };

         records.push(rr::Record::from_rdata(
            rr::Name::from(name),
            u32::try_from(time::Duration::from_hours(1).as_secs()).expect("ttl must fit in u32"),
            rdata,
         ));
      }

      if records.is_empty() {
         metadata.response_code = op::ResponseCode::NXDomain;
      }

      response_handle
         .send_response(
            zone_handler::MessageResponseBuilder::from_message_request(request).build(
               metadata,
               records.iter(),
               [],
               [],
               [],
            ),
         )
         .await
         .unwrap_or_else(|error| {
            tracing::warn!(%error, "Failed to send dns response");
            hserver::ResponseInfo::from(op::Header {
               metadata,
               counts: op::HeaderCounts::default(),
            })
         })
   }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
   #[error("failed to bind '{address}'")]
   Bind {
      address: net::SocketAddr,
      #[source]
      source:  io::Error,
   },

   #[error("dns server task failed")]
   Server(#[source] hickory_net::NetError),
}

/// Binds the anycast DNS listener on `fd67::1:53` and returns a receiver of
/// main-loop queries.
pub async fn listen(
   join_set: &mut task::JoinSet<Result<(), Error>>,
) -> Result<mpsc::UnboundedReceiver<Query>, Error> {
   let (sender, receiver) = mpsc::unbounded_channel();

   let mut server = hserver::Server::new(Handler { queries: sender });

   server.register_socket({
      let address = net::SocketAddr::new(
         net::IpAddr::V6(net::Ipv6Addr::from({
            let mut octets = [0; _];
            octets[address::VPN_PREFIX_RANGE].copy_from_slice(&address::VPN_PREFIX);
            *octets.last_mut().expect("address array must not be empty") = 0x01;
            octets
         })),
         67,
      );

      tokio_net::UdpSocket::bind(&address)
         .await
         .map_err(|source| Error::Bind { address, source })
   }?);

   join_set.spawn(async move { server.block_until_done().await.map_err(Error::Server) });

   Ok(receiver)
}
