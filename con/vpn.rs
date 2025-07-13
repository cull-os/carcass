use std::net;

use async_trait::async_trait;
use libp2p::{
   self as p2p,
   futures::{
      self,
      AsyncReadExt as _,
      AsyncWriteExt as _,
   },
   request_response as p2p_rr,
};
use rustc_hash::FxHashSet;
use tokio::io;

use crate::address;

/// VPN protocol for sending IP packets between peers.
pub type Behaviour = p2p_rr::Behaviour<Codec>;

#[derive(Debug, Clone, Copy)]
pub struct Codec;

#[async_trait]
impl p2p_rr::Codec for Codec {
   type Protocol = String;

   /// Raw IP packet
   type Request = Vec<u8>;

   /// No response needed for VPN packets
   type Response = ();

   async fn read_request<T: futures::AsyncRead + Unpin + Send>(
      &mut self,
      _: &Self::Protocol,
      read: &mut T,
   ) -> io::Result<Self::Request> {
      let mut len = [0_u8; 2];
      read.read_exact(&mut len).await?;

      let len = u16::from_le_bytes(len) as usize;

      let mut packet = Vec::with_capacity(len);
      read.read_exact(&mut packet).await?;

      let packet_len = packet.len();
      packet[packet_len..len].fill(0);

      Ok(packet)
   }

   async fn write_request<T: futures::AsyncWrite + Unpin + Send>(
      &mut self,
      _: &Self::Protocol,
      io: &mut T,
      request: Self::Request,
   ) -> io::Result<()> {
      let len = u16::try_from(request.len())
         .expect("ip packet len ")
         .to_le_bytes();
      io.write_all(&len).await?;

      io.write_all(&request).await?;

      io.flush().await?;

      Ok(())
   }

   async fn read_response<T: futures::AsyncRead + Unpin + Send>(
      &mut self,
      _: &Self::Protocol,
      _read: &mut T,
   ) -> io::Result<Self::Response> {
      // VPN packets are fire-and-forget, no response.
      Ok(())
   }

   async fn write_response<T: futures::AsyncWrite + Unpin + Send>(
      &mut self,
      _: &Self::Protocol,
      io: &mut T,
      _resonse: Self::Response,
   ) -> io::Result<()> {
      // Send empty response to ACK.
      io.flush().await?;

      Ok(())
   }
}

#[must_use]
pub fn new() -> Behaviour {
   todo!()
}

pub struct Router {
   address_map:  address::Map,
   peer_streams: FxHashSet<p2p::PeerId>,
}

impl Router {
   #[must_use]
   pub fn new() -> Self {
      Self {
         address_map:  address::Map::new(),
         peer_streams: FxHashSet::default(),
      }
   }

   pub fn register_peer(&mut self, peer_id: p2p::PeerId) {
      self.address_map.register(peer_id);
      self.peer_streams.insert(peer_id);
   }

   #[must_use]
   #[expect(
      clippy::missing_asserts_for_indexing,
      reason = "clippy is too dump to see if guards"
   )]
   pub fn peer_id_of(&self, packet: &[u8]) -> Option<p2p::PeerId> {
      if packet.len() < 20 {
         return None;
      }

      let version = (packet[0] >> 4_usize) & 0x0F;

      match version {
         // IPv4: Destination at bytes 16-19.
         4 => {
            let destination = net::Ipv4Addr::from([packet[16], packet[17], packet[18], packet[19]]);
            self.address_map.get_peer_by_v4(&destination)
         },

         // IPv6: Destination at bytes 24-39.
         6 => {
            if packet.len() < 40 {
               return None;
            }

            let destination = net::Ipv6Addr::from(
               <[u8; 16]>::try_from(&packet[24..40]).expect("size was statically checked"),
            );

            self.address_map.get_peer_by_v6(&destination)
         },

         _ => None, // Unknown IP version.
      }
   }
}
