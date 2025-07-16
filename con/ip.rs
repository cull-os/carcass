use std::{
   collections::VecDeque,
   mem,
   pin::pin,
   sync::Arc,
   task,
};

use derive_more::Deref;
use libp2p::{
   self as p2p,
   core::{
      self as p2p_core,
      transport as p2p_core_transport,
      upgrade as p2p_core_upgrade,
   },
   futures::{
      AsyncReadExt as _,
      AsyncWriteExt as _,
      future::BoxFuture,
   },
   swarm::{
      self as p2p_swarm,
      dial_opts as p2p_swarm_dial_opts,
      handler as p2p_swarm_handler,
   },
};
use ringbuf::traits::{
   Consumer as _,
   Producer as _,
   Split as _,
};
use rustc_hash::{
   FxBuildHasher,
   FxHashMap,
};
use tokio::io;

const PROTOCOL: p2p_swarm::StreamProtocol = p2p_swarm::StreamProtocol::new("/ip/0.0.1");

#[derive(Debug, Deref, Clone)]
pub struct Packet(Vec<u8>);

impl Packet {
   #[must_use]
   pub fn new(data: Vec<u8>) -> Self {
      Self(data)
   }

   pub async fn read_from(mut stream: p2p::Stream) -> io::Result<(p2p::Stream, Self)> {
      let mut len = [0_u8; 2];

      stream.read_exact(&mut len).await?;

      let len = u16::from_le_bytes(len) as usize;

      let mut data = Vec::with_capacity(len);
      stream.read_exact(&mut data).await?;

      if data.len() != len {
         return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "packet data size did not match packet length header",
         ));
      }

      Ok((stream, Self(data)))
   }

   pub async fn write_to(self, mut stream: p2p::Stream) -> io::Result<p2p::Stream> {
      let len = u16::try_from(self.len())
         .map_err(|_| {
            io::Error::new(
               io::ErrorKind::InvalidInput,
               "packet too large, must fit in u16::MAX bytes",
            )
         })?
         .to_le_bytes();

      stream.write_all(&len).await?;
      stream.write_all(&self).await?;
      stream.flush().await?;

      Ok(stream)
   }
}

enum HandlerAction {
   Reading(BoxFuture<'static, io::Result<(p2p::Stream, Packet)>>),
   Writing(BoxFuture<'static, io::Result<p2p::Stream>>),
   Idle(Option<p2p_swarm::Stream>),
}

impl Default for HandlerAction {
   fn default() -> Self {
      Self::Idle(None)
   }
}

const PACKET_BUFFER_SIZE: usize = 256;
type PacketProducer = ringbuf::CachingProd<Arc<ringbuf::StaticRb<Packet, PACKET_BUFFER_SIZE>>>;
type PacketConsumer = ringbuf::CachingCons<Arc<ringbuf::StaticRb<Packet, PACKET_BUFFER_SIZE>>>;

pub struct Handler {
   consumer: PacketConsumer,
   action:   HandlerAction,
}

impl Handler {
   fn new(consumer: PacketConsumer) -> Self {
      Handler {
         consumer,
         action: HandlerAction::default(),
      }
   }
}

impl p2p_swarm::ConnectionHandler for Handler {
   type FromBehaviour = ();

   type ToBehaviour = Packet;

   type InboundProtocol = p2p_core_upgrade::ReadyUpgrade<p2p::StreamProtocol>;

   type OutboundProtocol = p2p_core_upgrade::ReadyUpgrade<p2p::StreamProtocol>;

   type InboundOpenInfo = ();

   type OutboundOpenInfo = ();

   fn listen_protocol(
      &self,
   ) -> p2p_swarm::SubstreamProtocol<Self::InboundProtocol, Self::InboundOpenInfo> {
      p2p_swarm::SubstreamProtocol::new(p2p_core_upgrade::ReadyUpgrade::new(PROTOCOL), ())
   }

   fn on_behaviour_event(&mut self, _event: Self::FromBehaviour) {}

   #[tracing::instrument(
      level = "trace",
      name = "ConnectionHandler::on_connection_event",
      skip(self)
   )]
   fn on_connection_event(
      &mut self,
      event: p2p_swarm_handler::ConnectionEvent<
         Self::InboundProtocol,
         Self::OutboundProtocol,
         (),
         (),
      >,
   ) {
      let stream_new = match event {
         p2p_swarm_handler::ConnectionEvent::FullyNegotiatedInbound(new) => new.protocol,
         p2p_swarm_handler::ConnectionEvent::FullyNegotiatedOutbound(new) => new.protocol,
         _ => return,
      };

      match self.action {
         HandlerAction::Reading(_) => {},
         HandlerAction::Writing(_) => {},

         HandlerAction::Idle(ref mut stream) => {
            *stream = Some(stream_new);
         },
      }
   }

   #[tracing::instrument(level = "trace", name = "ConnectionHandler::poll", skip(self, context))]
   fn poll(
      &mut self,
      context: &mut task::Context<'_>,
   ) -> task::Poll<
      p2p_swarm::ConnectionHandlerEvent<
         Self::OutboundProtocol,
         Self::OutboundOpenInfo,
         Self::ToBehaviour,
      >,
   > {
      use p2p_swarm::ConnectionHandlerEvent::{
         NotifyBehaviour,
         OutboundSubstreamRequest,
      };
      use task::Poll::{
         Pending,
         Ready,
      };

      match mem::take(&mut self.action) {
         HandlerAction::Reading(mut read) => {
            let Ready(result) = pin!(&mut read).poll(context) else {
               self.action = HandlerAction::Reading(read);
               return Pending;
            };

            match result {
               Ok((stream, packet)) => {
                  self.action = HandlerAction::Idle(Some(stream));

                  return Ready(NotifyBehaviour(packet));
               },

               Err(error) => {
                  tracing::warn!("Failed to read packet from stream: {error}");
               },
            }
         },

         HandlerAction::Writing(mut write) => {
            let Ready(result) = pin!(&mut write).poll(context) else {
               self.action = HandlerAction::Writing(write);
               return Pending;
            };

            match result {
               Ok(stream) => self.action = HandlerAction::Idle(Some(stream)),

               Err(error) => {
                  tracing::warn!("Failed to write packet to stream: {error}");
               },
            }
         },

         HandlerAction::Idle(Some(stream)) => {
            if let Some(packet) = self.consumer.try_pop() {
               self.action = HandlerAction::Writing(Box::pin(packet.write_to(stream)));
            } else {
               self.action = HandlerAction::Reading(Box::pin(Packet::read_from(stream)));
            }
         },

         HandlerAction::Idle(None) => {
            return Ready(OutboundSubstreamRequest {
               protocol: self.listen_protocol(),
            });
         },
      }

      Pending
   }
}

pub trait Policy = FnMut(&p2p::PeerId) -> Result<(), p2p_swarm::ConnectionDenied> + 'static;

pub struct Behaviour<P: Policy> {
   inbound_policy: P,
   inbound_queue:  VecDeque<Packet>,

   outbound_handlers: FxHashMap<p2p::PeerId, PacketProducer>,
   outbound_packets:  FxHashMap<p2p::PeerId, Vec<Packet>>,
}

impl<P: Policy> Behaviour<P> {
   pub fn new(inbound_policy: P) -> Self {
      Self {
         inbound_policy,
         inbound_queue: VecDeque::new(),

         outbound_handlers: FxHashMap::with_hasher(FxBuildHasher),
         outbound_packets: FxHashMap::with_hasher(FxBuildHasher),
      }
   }

   pub fn send(&mut self, peer_id: &p2p::PeerId, packet: Packet) {
      let Some(producer) = self.outbound_handlers.get_mut(peer_id) else {
         self
            .outbound_packets
            .entry(*peer_id)
            .or_default()
            .push(packet);
         return;
      };

      let _ = producer.try_push(packet);
   }
}

impl<P: Policy> p2p_swarm::NetworkBehaviour for Behaviour<P> {
   type ConnectionHandler = Handler;

   type ToSwarm = Packet;

   fn handle_established_inbound_connection(
      &mut self,
      _connection_id: p2p_swarm::ConnectionId,
      peer_id: p2p::PeerId,
      _local_addr: &p2p::Multiaddr,
      _remote_addr: &p2p::Multiaddr,
   ) -> Result<Handler, p2p_swarm::ConnectionDenied> {
      (self.inbound_policy)(&peer_id)?;

      let (mut producer, consumer) = ringbuf::StaticRb::default().split();

      if let Some(packets) = self.outbound_packets.remove(&peer_id) {
         producer.push_iter(packets.into_iter());
      }

      self.outbound_handlers.insert(peer_id, producer);

      Ok(Handler::new(consumer))
   }

   fn handle_established_outbound_connection(
      &mut self,
      _connection_id: p2p_swarm::ConnectionId,
      peer_id: p2p::PeerId,
      _addr: &p2p::Multiaddr,
      _role_override: p2p_core::Endpoint,
      _port_use: p2p_core_transport::PortUse,
   ) -> Result<Handler, p2p_swarm::ConnectionDenied> {
      let (mut producer, consumer) = ringbuf::StaticRb::default().split();

      if let Some(packets) = self.outbound_packets.remove(&peer_id) {
         producer.push_iter(packets.into_iter());
      }

      self.outbound_handlers.insert(peer_id, producer);

      Ok(Handler::new(consumer))
   }

   fn on_swarm_event(&mut self, _event: p2p_swarm::FromSwarm) {}

   fn on_connection_handler_event(
      &mut self,
      _peer_id: p2p::PeerId,
      _connection_id: p2p_swarm::ConnectionId,
      packet: Packet,
   ) {
      self.inbound_queue.push_back(packet);
   }

   #[tracing::instrument(level = "trace", name = "NetworkBehaviour::poll", skip(self, _context))]
   fn poll(
      &mut self,
      _context: &mut task::Context<'_>,
   ) -> task::Poll<p2p_swarm::ToSwarm<Packet, ()>> {
      // Check for outbound packets and dial.
      #[expect(clippy::iter_over_hash_type)]
      for outbound_peer_id in self.outbound_packets.keys() {
         if !self.outbound_handlers.contains_key(outbound_peer_id) {
            return task::Poll::Ready(p2p_swarm::ToSwarm::Dial {
               opts: p2p_swarm_dial_opts::DialOpts::peer_id(*outbound_peer_id).build(),
            });
         }
      }

      // Then check for incoming packets to emit.
      match self.inbound_queue.pop_front() {
         Some(packet) => task::Poll::Ready(p2p_swarm::ToSwarm::GenerateEvent(packet)),
         None => task::Poll::Pending,
      }
   }
}
