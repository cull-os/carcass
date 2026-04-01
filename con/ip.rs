use std::{
   collections::VecDeque,
   io,
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
   BytesMut,
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
      SinkExt as _,
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
pub struct Packet(pub Vec<u8>);

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
      if src.len() < 2 {
         return Ok(None);
      }

      let len = u16::from_le_bytes([src[0], src[1]]) as usize;

      if src.len() < 2 + len {
         src.reserve(2 + len - src.len());
         return Ok(None);
      }

      src.advance(2);
      let data = src.split_to(len);

      Ok(Some(Packet(data.to_vec())))
   }
}

type FramedStream = Framed<p2p::Stream, PacketCodec>;

enum InboundState {
   WaitingInput(FramedStream),
   Closing(FramedStream),
}

enum OutboundState {
   WaitingOutput(FramedStream),
   PendingSend(FramedStream, Packet),
   PendingFlush(FramedStream),
}

const MAX_SUBSTREAM_ATTEMPTS: usize = 5;

const PACKET_BUFFER_SIZE: usize = 256;
type PacketProducer = ringbuf::CachingProd<Arc<ringbuf::StaticRb<Packet, PACKET_BUFFER_SIZE>>>;
type PacketConsumer = ringbuf::CachingCons<Arc<ringbuf::StaticRb<Packet, PACKET_BUFFER_SIZE>>>;

pub enum Handler {
   Enabled {
      consumer:              PacketConsumer,
      inbound:               Option<InboundState>,
      outbound:              Option<OutboundState>,
      outbound_establishing: bool,
      inbound_attempts:      usize,
      outbound_attempts:     usize,
   },
   Disabled,
}

impl Handler {
   fn new(consumer: PacketConsumer) -> Self {
      Handler::Enabled {
         consumer,
         inbound: None,
         outbound: None,
         outbound_establishing: false,
         inbound_attempts: 0,
         outbound_attempts: 0,
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
      let Handler::Enabled {
         ref mut inbound,
         ref mut outbound,
         ref mut outbound_establishing,
         ref mut inbound_attempts,
         ref mut outbound_attempts,
         ..
      } = *self
      else {
         return;
      };

      match event {
         p2p_swarm_handler::ConnectionEvent::FullyNegotiatedInbound(new) => {
            *inbound = Some(InboundState::WaitingInput(Framed::new(
               new.protocol,
               PacketCodec,
            )));

            *inbound_attempts += 1;
            if *inbound_attempts >= MAX_SUBSTREAM_ATTEMPTS {
               tracing::warn!("Maximum inbound substream attempts exceeded.");
               *self = Handler::Disabled;
            }
         },
         p2p_swarm_handler::ConnectionEvent::FullyNegotiatedOutbound(new) => {
            *outbound = Some(OutboundState::WaitingOutput(Framed::new(
               new.protocol,
               PacketCodec,
            )));
            *outbound_establishing = false;
         },
         p2p_swarm_handler::ConnectionEvent::DialUpgradeError(_) => {
            *outbound_establishing = false;
            *outbound_attempts += 1;
            if *outbound_attempts >= MAX_SUBSTREAM_ATTEMPTS {
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

      let Handler::Enabled {
         ref mut consumer,
         ref mut inbound,
         ref mut outbound,
         ref mut outbound_establishing,
         ..
      } = *self
      else {
         return Pending;
      };

      // Request outbound substream if needed.
      if outbound.is_none() && !*outbound_establishing && !consumer.is_empty() {
         *outbound_establishing = true;
         return Ready(OutboundSubstreamRequest {
            protocol: p2p_swarm::SubstreamProtocol::new(
               p2p_core_upgrade::ReadyUpgrade::new(PROTOCOL),
               (),
            ),
         });
      }

      // Process outbound.
      loop {
         match outbound.take() {
            Some(OutboundState::WaitingOutput(substream)) => {
               if let Some(packet) = consumer.try_pop() {
                  *outbound = Some(OutboundState::PendingSend(substream, packet));
                  continue;
               }

               *outbound = Some(OutboundState::WaitingOutput(substream));
               break;
            },
            Some(OutboundState::PendingSend(mut substream, packet)) => {
               match Pin::new(&mut substream).poll_ready(cx) {
                  Ready(Ok(())) => {
                     match Pin::new(&mut substream).start_send(&packet) {
                        Ok(()) => {
                           *outbound = Some(OutboundState::PendingFlush(substream));
                        },
                        Err(error) => {
                           tracing::debug!("Failed to send packet on outbound stream: {error}");
                           *outbound = None;
                           break;
                        },
                     }
                  },
                  Ready(Err(error)) => {
                     tracing::debug!("Failed to send packet on outbound stream: {error}");
                     *outbound = None;
                     break;
                  },
                  Pending => {
                     *outbound = Some(OutboundState::PendingSend(substream, packet));
                     break;
                  },
               }
            },
            Some(OutboundState::PendingFlush(mut substream)) => {
               match Pin::new(&mut substream).poll_flush(cx) {
                  Ready(Ok(())) => {
                     *outbound = Some(OutboundState::WaitingOutput(substream));
                  },
                  Ready(Err(error)) => {
                     tracing::debug!("Failed to flush outbound stream: {error}");
                     *outbound = None;
                     break;
                  },
                  Pending => {
                     *outbound = Some(OutboundState::PendingFlush(substream));
                     break;
                  },
               }
            },
            None => break,
         }
      }

      // Process inbound.
      loop {
         match inbound.take() {
            Some(InboundState::WaitingInput(mut substream)) => {
               match substream.poll_next_unpin(cx) {
                  Ready(Some(Ok(packet))) => {
                     *inbound = Some(InboundState::WaitingInput(substream));
                     return Ready(NotifyBehaviour(packet));
                  },
                  Ready(Some(Err(error))) => {
                     tracing::debug!("Failed to read from inbound stream: {error}");
                     *inbound = Some(InboundState::Closing(substream));
                  },
                  Ready(None) => {
                     tracing::debug!("Inbound stream closed by remote.");
                     *inbound = Some(InboundState::Closing(substream));
                  },
                  Pending => {
                     *inbound = Some(InboundState::WaitingInput(substream));
                     break;
                  },
               }
            },
            Some(InboundState::Closing(mut substream)) => {
               match Pin::new(&mut substream).poll_close(cx) {
                  Ready(_) => {
                     *inbound = None;
                     break;
                  },
                  Pending => {
                     *inbound = Some(InboundState::Closing(substream));
                     break;
                  },
               }
            },
            None => break,
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
