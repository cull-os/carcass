use std::{
   collections::VecDeque,
   mem,
   pin::{
      Pin,
      pin,
   },
   sync::Arc,
   task,
   time::Duration,
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
   Observer as _,
   Producer as _,
   Split as _,
};
use rustc_hash::{
   FxBuildHasher,
   FxHashMap,
};
use tokio::{
   io,
   time as tokio_time,
};

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

      (&mut stream)
         .take(len as u64)
         .read_to_end(&mut data)
         .await?;

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

#[derive(Default)]
enum ReadState {
   Reading(BoxFuture<'static, io::Result<(p2p::Stream, Packet)>>),
   Connected(p2p::Stream),
   Backoff(Pin<Box<tokio_time::Sleep>>),
   #[default]
   Disconnected,
}

#[derive(Default)]
enum WriteState {
   Writing(BoxFuture<'static, io::Result<p2p::Stream>>),
   Connected(p2p::Stream),
   Backoff(Pin<Box<tokio_time::Sleep>>),
   Requested,
   #[default]
   Disconnected,
}

const PACKET_BUFFER_SIZE: usize = 256;
type PacketProducer = ringbuf::CachingProd<Arc<ringbuf::StaticRb<Packet, PACKET_BUFFER_SIZE>>>;
type PacketConsumer = ringbuf::CachingCons<Arc<ringbuf::StaticRb<Packet, PACKET_BUFFER_SIZE>>>;

pub struct Handler {
   consumer: PacketConsumer,

   read:       ReadState,
   read_tries: u32,

   write:       WriteState,
   write_tries: u32,
}

impl Handler {
   fn new(consumer: PacketConsumer) -> Self {
      Handler {
         consumer,
         read: ReadState::Disconnected,
         read_tries: 0,
         write: WriteState::Disconnected,
         write_tries: 0,
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
      match event {
         p2p_swarm_handler::ConnectionEvent::FullyNegotiatedInbound(new) => {
            if let ReadState::Disconnected = self.read {
               self.read = ReadState::Connected(new.protocol);
            }
         },
         p2p_swarm_handler::ConnectionEvent::FullyNegotiatedOutbound(new) => {
            if let WriteState::Requested = self.write {
               self.write = WriteState::Connected(new.protocol);
            }
         },
         p2p_swarm_handler::ConnectionEvent::DialUpgradeError(_) => {
            if let WriteState::Requested = self.write {
               self.write = WriteState::Disconnected;
            }
         },
         _ => {},
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

      'read: {
         match self.read {
            ReadState::Reading(ref mut read) => {
               let Ready(result) = pin!(read).poll(context) else {
                  break 'read;
               };

               let Ok((stream, packet)) = result else {
                  tracing::warn!(
                     "Failed to read packet from stream: {error}",
                     error = result.unwrap_err(),
                  );
                  self.read_tries += 1;
                  self.read = ReadState::Backoff(Box::pin(tokio_time::sleep(Duration::from_secs(
                     1_u64.checked_shl(self.read_tries).unwrap_or(u64::MAX),
                  ))));
                  break 'read;
               };

               self.read_tries = 0;
               self.read = ReadState::Connected(stream);
               return Ready(NotifyBehaviour(packet));
            },

            ReadState::Connected(..) => {
               let ReadState::Connected(stream) = mem::take(&mut self.read) else {
                  unreachable!();
               };

               self.read = ReadState::Reading(Box::pin(Packet::read_from(stream)));
            },

            ReadState::Backoff(ref mut sleep) => {
               let Ready(()) = sleep.as_mut().poll(context) else {
                  break 'read;
               };

               self.read = ReadState::Disconnected;
            },

            ReadState::Disconnected => {},
         }
      }

      'write: {
         match self.write {
            WriteState::Writing(ref mut write) => {
               let Ready(result) = pin!(write).poll(context) else {
                  break 'write;
               };

               let Ok(stream) = result else {
                  tracing::warn!("Failed to write packet to stream: {}", result.unwrap_err());
                  self.write_tries += 1;
                  self.write = WriteState::Backoff(Box::pin(tokio_time::sleep(
                     Duration::from_secs(1_u64.checked_shl(self.write_tries).unwrap_or(u64::MAX)),
                  )));
                  break 'write;
               };

               self.write_tries = 0;
               self.write = WriteState::Connected(stream);
            },

            WriteState::Connected(..) => {
               if let Some(packet) = self.consumer.try_pop() {
                  let WriteState::Connected(stream) = mem::take(&mut self.write) else {
                     unreachable!();
                  };

                  self.write = WriteState::Writing(Box::pin(packet.write_to(stream)));
               }
            },

            WriteState::Backoff(ref mut sleep) => {
               let Ready(()) = sleep.as_mut().poll(context) else {
                  break 'write;
               };

               self.write = WriteState::Disconnected;
            },

            WriteState::Requested => {},

            WriteState::Disconnected => {
               if !self.consumer.is_empty() {
                  self.write = WriteState::Requested;
                  return Ready(OutboundSubstreamRequest {
                     protocol: self.listen_protocol(),
                  });
               }
            },
         }
      }

      Pending
   }
}

pub trait Policy = FnMut(&p2p::PeerId) -> Result<(), p2p_swarm::ConnectionDenied> + 'static;

pub struct Behaviour<P: Policy> {
   queued_events: VecDeque<p2p_swarm::ToSwarm<Packet, ()>>,

   inbound_policy: P,

   outbound_handlers: FxHashMap<p2p::PeerId, PacketProducer>,
   outbound_buffers:  FxHashMap<p2p::PeerId, (PacketProducer, PacketConsumer)>,
}

impl<P: Policy> Behaviour<P> {
   pub fn new(inbound_policy: P) -> Self {
      Self {
         queued_events: VecDeque::new(),

         inbound_policy,

         outbound_handlers: FxHashMap::with_hasher(FxBuildHasher),
         outbound_buffers: FxHashMap::with_hasher(FxBuildHasher),
      }
   }

   pub fn send(&mut self, peer_id: &p2p::PeerId, packet: Packet) {
      let producer = if let Some(producer) = self.outbound_handlers.get_mut(peer_id) {
         producer
      } else if let Some(&mut (ref mut producer, _)) = self.outbound_buffers.get_mut(peer_id) {
         producer
      } else {
         self.queued_events.push_back(p2p_swarm::ToSwarm::Dial {
            opts: p2p_swarm_dial_opts::DialOpts::peer_id(*peer_id).build(),
         });

         &mut self
            .outbound_buffers
            .entry(*peer_id)
            .or_insert_with(|| ringbuf::StaticRb::default().split())
            .0
      };

      let Ok(()) = producer.try_push(packet) else {
         tracing::warn!("Packet buffer full for peer '{peer_id}', dropping packet.");
         return;
      };
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

      let (producer, consumer) = self
         .outbound_buffers
         .remove(&peer_id)
         .unwrap_or_else(|| ringbuf::StaticRb::default().split());

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
      let (producer, consumer) = self
         .outbound_buffers
         .remove(&peer_id)
         .unwrap_or_else(|| ringbuf::StaticRb::default().split());

      self.outbound_handlers.insert(peer_id, producer);

      Ok(Handler::new(consumer))
   }

   fn on_swarm_event(&mut self, event: p2p_swarm::FromSwarm) {
      match event {
         p2p_swarm::FromSwarm::ConnectionClosed(closed) if closed.remaining_established == 0 => {
            self.outbound_handlers.remove(&closed.peer_id);
         },
         _ => {},
      }
   }

   fn on_connection_handler_event(
      &mut self,
      _peer_id: p2p::PeerId,
      _connection_id: p2p_swarm::ConnectionId,
      packet: Packet,
   ) {
      self
         .queued_events
         .push_back(p2p_swarm::ToSwarm::GenerateEvent(packet));
   }

   #[tracing::instrument(level = "trace", name = "NetworkBehaviour::poll", skip(self, _context))]
   fn poll(
      &mut self,
      _context: &mut task::Context<'_>,
   ) -> task::Poll<p2p_swarm::ToSwarm<Packet, ()>> {
      match self.queued_events.pop_front() {
         Some(event) => task::Poll::Ready(event),
         None => task::Poll::Pending,
      }
   }
}
