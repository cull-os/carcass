use std::{
   io,
   net,
   time,
};

use hickory_server::{
   authority,
   proto::{
      op,
      rr::{
         self,
         rdata,
      },
   },
   server as hserver,
};
use libp2p as p2p;
use tokio::{
   net as tokio_net,
   sync::{
      mpsc,
      oneshot,
   },
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

      let peer_id = match (labels.next(), labels.next(), labels.next()) {
         (Some(peer), Some(zone), None) if zone.eq_ignore_ascii_case(b"internal") => {
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
   async fn handle_request<R: hserver::ResponseHandler>(
      &self,
      request: &hserver::Request,
      mut response_handle: R,
   ) -> hserver::ResponseInfo {
      let mut header = op::Header::response_from_request(request.header());
      header.set_authoritative(true);

      let mut records = Vec::new();
      for query in request.queries() {
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
         header.set_response_code(op::ResponseCode::NXDomain);
      }

      response_handle
         .send_response(
            authority::MessageResponseBuilder::from_message_request(request).build(
               header,
               records.iter(),
               [],
               [],
               [],
            ),
         )
         .await
         .unwrap_or_else(|error| {
            tracing::warn!(%error, "Failed to send dns response");
            hserver::ResponseInfo::from(header)
         })
   }
}

#[derive(Debug, thiserror::Error)]
pub enum ListenError {
   #[error("failed to bind '{address}'")]
   Bind {
      address: net::SocketAddr,
      #[source]
      source:  io::Error,
   },
}

/// Binds the anycast DNS listener on `fd67::1:53` and returns a receiver of
/// main-loop queries.
pub async fn listen() -> Result<mpsc::UnboundedReceiver<Query>, ListenError> {
   let (sender, receiver) = mpsc::unbounded_channel();

   let mut server = hserver::ServerFuture::new(Handler { queries: sender });

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
         .map_err(|source| ListenError::Bind { address, source })
   }?);

   tokio::spawn(async move {
      if let Err(error) = server.block_until_done().await {
         tracing::error!(%error, "Dns server exited with error");
      }
   });

   Ok(receiver)
}
