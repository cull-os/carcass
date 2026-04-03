use std::{
   collections::VecDeque,
   convert::Infallible,
   io,
   mem,
   pin::Pin,
   sync::Arc,
   task,
};

use asynchronous_codec::{
   Decoder,
   Encoder,
   Framed,
};
use bytes::{
   Buf as _,
   BufMut as _,
   Bytes,
   BytesMut,
};
use derive_more::Deref;
use either::Either;
use libp2p::{
   self as p2p,
   core::{
      self as p2p_core,
      transport as p2p_core_transport,
      upgrade as p2p_core_upgrade,
   },
   futures::{
      Sink as _,
      StreamExt as _,
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

const PROTOCOL: p2p_swarm::StreamProtocol = p2p_swarm::StreamProtocol::new("/ip/0.0.1");

#[derive(Debug, Deref, Clone)]
pub struct Packet(pub Bytes);

struct PacketCodec;

impl Encoder for PacketCodec {
   type Item<'a> = &'a Packet;
   type Error = io::Error;

   fn encode(&mut self, item: Self::Item<'_>, dst: &mut BytesMut) -> Result<(), Self::Error> {
      let len = u16::try_from(item.len()).map_err(|_| {
         io::Error::new(
            io::ErrorKind::InvalidInput,
            "packet too large, must fit in u16::MAX bytes",
         )
      })?;

      dst.reserve(2 + item.len());
      dst.put_u16_le(len);
      dst.extend_from_slice(item);

      Ok(())
   }
}

impl Decoder for PacketCodec {
   type Item = Packet;
   type Error = io::Error;

   fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
      let Some(len_bytes) = src.get(..2) else {
         return Ok(None);
      };

      let len =
         u16::from_le_bytes(<[u8; 2]>::try_from(len_bytes).expect("length checked")) as usize;

      if src.len() < 2 + len {
         return Ok(None);
      }

      src.advance(2);
      Ok(Some(Packet(src.split_to(len).freeze())))
   }
}

type FramedStream = Framed<p2p::Stream, PacketCodec>;

enum InboundState {
   Disconnected,
   WaitingInput(FramedStream),
   Closing(FramedStream),
   Poisoned,
}

enum OutboundState {
   Disconnected,
   Requested,
   WaitingOutput(FramedStream),
   PendingSend(FramedStream, Packet),
   PendingFlush(FramedStream),
   Poisoned,
}

const MAX_SUBSTREAM_ATTEMPTS: usize = 5;

const PACKET_BUFFER_SIZE: usize = 256;
type PacketProducer = ringbuf::CachingProd<Arc<ringbuf::StaticRb<Packet, PACKET_BUFFER_SIZE>>>;
type PacketConsumer = ringbuf::CachingCons<Arc<ringbuf::StaticRb<Packet, PACKET_BUFFER_SIZE>>>;

pub struct EnabledHandler {
   consumer:          PacketConsumer,
   inbound:           InboundState,
   inbound_attempts:  usize,
   outbound:          OutboundState,
   outbound_attempts: usize,
}

#[expect(clippy::large_enum_variant)]
pub enum Handler {
   Enabled(EnabledHandler),
   Disabled,
}

impl Handler {
   fn new(consumer: PacketConsumer) -> Self {
      Handler::Enabled(EnabledHandler {
         consumer,
         inbound: InboundState::Disconnected,
         inbound_attempts: 0,
         outbound: OutboundState::Disconnected,
         outbound_attempts: 0,
      })
   }
}

impl p2p_swarm::ConnectionHandler for Handler {
   type FromBehaviour = Infallible;

   type ToBehaviour = Packet;

   type InboundProtocol =
      Either<p2p_core_upgrade::ReadyUpgrade<p2p::StreamProtocol>, p2p_core_upgrade::DeniedUpgrade>;

   type OutboundProtocol = p2p_core_upgrade::ReadyUpgrade<p2p::StreamProtocol>;

   type InboundOpenInfo = ();

   type OutboundOpenInfo = ();

   fn listen_protocol(
      &self,
   ) -> p2p_swarm::SubstreamProtocol<Self::InboundProtocol, Self::InboundOpenInfo> {
      p2p_swarm::SubstreamProtocol::new(
         match *self {
            Handler::Enabled(..) => Either::Left(p2p_core_upgrade::ReadyUpgrade::new(PROTOCOL)),
            Handler::Disabled => Either::Right(p2p_core_upgrade::DeniedUpgrade),
         },
         (),
      )
   }

   fn connection_keep_alive(&self) -> bool {
      matches!(self, Handler::Enabled(..))
   }

   fn on_behaviour_event(&mut self, event: Self::FromBehaviour) {
      match event {}
   }

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
         Self::InboundOpenInfo,
         Self::OutboundOpenInfo,
      >,
   ) {
      let Handler::Enabled(ref mut handler) = *self else {
         return;
      };

      match event {
         p2p_swarm_handler::ConnectionEvent::FullyNegotiatedInbound(new) => {
            #[expect(clippy::absolute_paths)]
            let p2p::futures::future::Either::Left(stream) = new.protocol;

            handler.inbound = InboundState::WaitingInput(Framed::new(stream, PacketCodec));

            handler.inbound_attempts += 1;
            if handler.inbound_attempts >= MAX_SUBSTREAM_ATTEMPTS {
               tracing::warn!("Maximum inbound substream attempts exceeded.");
               *self = Handler::Disabled;
            }
         },
         p2p_swarm_handler::ConnectionEvent::FullyNegotiatedOutbound(new) => {
            handler.outbound = OutboundState::WaitingOutput(Framed::new(new.protocol, PacketCodec));
         },
         p2p_swarm_handler::ConnectionEvent::DialUpgradeError(_) => {
            handler.outbound = OutboundState::Disconnected;
            handler.outbound_attempts += 1;
            if handler.outbound_attempts >= MAX_SUBSTREAM_ATTEMPTS {
               tracing::warn!("Maximum outbound substream attempts exceeded.");
               *self = Handler::Disabled;
            }
         },
         _ => {},
      }
   }

   #[tracing::instrument(level = "trace", name = "ConnectionHandler::poll", skip(self, cx))]
   fn poll(
      &mut self,
      cx: &mut task::Context<'_>,
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

      let Handler::Enabled(ref mut handler) = *self else {
         return Pending;
      };

      loop {
         match mem::replace(&mut handler.outbound, OutboundState::Poisoned) {
            OutboundState::Disconnected => {
               if !handler.consumer.is_empty() {
                  handler.outbound = OutboundState::Requested;
                  return Ready(OutboundSubstreamRequest {
                     protocol: p2p_swarm::SubstreamProtocol::new(
                        p2p_core_upgrade::ReadyUpgrade::new(PROTOCOL),
                        (),
                     ),
                  });
               }

               handler.outbound = OutboundState::Disconnected;
            },
            OutboundState::Requested => {
               handler.outbound = OutboundState::Requested;
            },
            OutboundState::WaitingOutput(substream) => {
               if let Some(packet) = handler.consumer.try_pop() {
                  handler.outbound = OutboundState::PendingSend(substream, packet);
                  continue;
               }

               handler.outbound = OutboundState::WaitingOutput(substream);
            },
            OutboundState::PendingSend(mut substream, packet) => {
               match Pin::new(&mut substream).poll_ready(cx) {
                  Ready(Ok(())) => {
                     match Pin::new(&mut substream).start_send(&packet) {
                        Ok(()) => {
                           handler.outbound = OutboundState::PendingFlush(substream);
                           continue;
                        },
                        Err(error) => {
                           tracing::debug!("Failed to send packet on outbound stream: {error}");
                           handler.outbound = OutboundState::Disconnected;
                        },
                     }
                  },
                  Ready(Err(error)) => {
                     tracing::debug!("Failed to send packet on outbound stream: {error}");
                     handler.outbound = OutboundState::Disconnected;
                  },
                  Pending => {
                     handler.outbound = OutboundState::PendingSend(substream, packet);
                  },
               }
            },
            OutboundState::PendingFlush(mut substream) => {
               match Pin::new(&mut substream).poll_flush(cx) {
                  Ready(Ok(())) => {
                     handler.outbound = OutboundState::WaitingOutput(substream);
                     continue;
                  },
                  Ready(Err(error)) => {
                     tracing::debug!("Failed to flush outbound stream: {error}");
                     handler.outbound = OutboundState::Disconnected;
                  },
                  Pending => {
                     handler.outbound = OutboundState::PendingFlush(substream);
                  },
               }
            },
            OutboundState::Poisoned => unreachable!(),
         }

         break;
      }

      loop {
         match mem::replace(&mut handler.inbound, InboundState::Poisoned) {
            InboundState::WaitingInput(mut substream) => {
               match substream.poll_next_unpin(cx) {
                  Ready(Some(Ok(packet))) => {
                     handler.inbound = InboundState::WaitingInput(substream);
                     return Ready(NotifyBehaviour(packet));
                  },
                  Ready(Some(Err(error))) => {
                     tracing::debug!("Failed to read from inbound stream: {error}");
                     handler.inbound = InboundState::Closing(substream);
                     continue;
                  },
                  Ready(None) => {
                     tracing::debug!("Inbound stream closed by remote.");
                     handler.inbound = InboundState::Closing(substream);
                     continue;
                  },
                  Pending => {
                     handler.inbound = InboundState::WaitingInput(substream);
                  },
               }
            },
            InboundState::Closing(mut substream) => {
               match Pin::new(&mut substream).poll_close(cx) {
                  Ready(Ok(())) => {
                     handler.inbound = InboundState::Disconnected;
                  },
                  Ready(Err(error)) => {
                     tracing::debug!("Failed to close inbound stream: {error}");
                     handler.inbound = InboundState::Disconnected;
                  },
                  Pending => {
                     handler.inbound = InboundState::Closing(substream);
                  },
               }
            },
            InboundState::Disconnected => {
               handler.inbound = InboundState::Disconnected;
            },
            InboundState::Poisoned => unreachable!(),
         }

         break;
      }

      Pending
   }
}

pub trait Policy = FnMut(&p2p::PeerId) -> Result<(), p2p_swarm::ConnectionDenied> + 'static;

pub struct Behaviour<P: Policy> {
   queued_events: VecDeque<p2p_swarm::ToSwarm<Packet, Infallible>>,

   inbound_policy: P,

   handlers: FxHashMap<p2p::PeerId, PacketProducer>,
   buffers:  FxHashMap<p2p::PeerId, (PacketProducer, PacketConsumer)>,
}

impl<P: Policy> Behaviour<P> {
   pub fn new(inbound_policy: P) -> Self {
      Self {
         queued_events: VecDeque::new(),

         inbound_policy,

         handlers: FxHashMap::with_hasher(FxBuildHasher),
         buffers: FxHashMap::with_hasher(FxBuildHasher),
      }
   }

   pub fn send(&mut self, peer_id: &p2p::PeerId, packet: Packet) {
      let producer = if let Some(producer) = self.handlers.get_mut(peer_id) {
         producer
      } else if let Some(&mut (ref mut producer, _)) = self.buffers.get_mut(peer_id) {
         self.queued_events.push_back(p2p_swarm::ToSwarm::Dial {
            opts: p2p_swarm_dial_opts::DialOpts::peer_id(*peer_id).build(),
         });
         producer
      } else {
         self.queued_events.push_back(p2p_swarm::ToSwarm::Dial {
            opts: p2p_swarm_dial_opts::DialOpts::peer_id(*peer_id).build(),
         });

         &mut self
            .buffers
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
   ) -> Result<Self::ConnectionHandler, p2p_swarm::ConnectionDenied> {
      (self.inbound_policy)(&peer_id)?;

      let (producer, consumer) = self
         .buffers
         .remove(&peer_id)
         .unwrap_or_else(|| ringbuf::StaticRb::default().split());

      self.handlers.insert(peer_id, producer);

      Ok(Handler::new(consumer))
   }

   fn handle_established_outbound_connection(
      &mut self,
      _connection_id: p2p_swarm::ConnectionId,
      peer_id: p2p::PeerId,
      _addr: &p2p::Multiaddr,
      _role_override: p2p_core::Endpoint,
      _port_use: p2p_core_transport::PortUse,
   ) -> Result<Self::ConnectionHandler, p2p_swarm::ConnectionDenied> {
      let (producer, consumer) = self
         .buffers
         .remove(&peer_id)
         .unwrap_or_else(|| ringbuf::StaticRb::default().split());

      self.handlers.insert(peer_id, producer);

      Ok(Handler::new(consumer))
   }

   fn on_swarm_event(&mut self, event: p2p_swarm::FromSwarm) {
      match event {
         p2p_swarm::FromSwarm::ConnectionClosed(closed) if closed.remaining_established == 0 => {
            self.handlers.remove(&closed.peer_id);
         },
         _ => {},
      }
   }

   fn on_connection_handler_event(
      &mut self,
      _peer_id: p2p::PeerId,
      _connection_id: p2p_swarm::ConnectionId,
      packet: p2p_swarm::THandlerOutEvent<Self>,
   ) {
      self
         .queued_events
         .push_back(p2p_swarm::ToSwarm::GenerateEvent(packet));
   }

   #[tracing::instrument(level = "trace", name = "NetworkBehaviour::poll", skip(self, _context))]
   fn poll(
      &mut self,
      _context: &mut task::Context<'_>,
   ) -> task::Poll<p2p_swarm::ToSwarm<Self::ToSwarm, p2p_swarm::THandlerInEvent<Self>>> {
      match self.queued_events.pop_front() {
         Some(event) => task::Poll::Ready(event),
         None => task::Poll::Pending,
      }
   }
}
