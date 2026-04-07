use std::{
   collections::VecDeque,
   io,
   mem,
   pin::Pin,
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
use ringbuf::{
   storage as ringbuf_storage,
   traits::{
      Consumer as _,
      Observer as _,
      Producer as _,
   },
};

const PROTOCOL: p2p_swarm::StreamProtocol = p2p_swarm::StreamProtocol::new("/ip/0.0.1");

#[derive(Debug, Deref, Clone)]
pub struct Packet(pub Bytes);

#[derive(Debug)]
pub enum Event {
   Packet(p2p::PeerId, Packet),
   DiscoverPeer(p2p::PeerId),
}

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

pub struct EnabledHandler {
   peer_id: p2p::PeerId,

   inbound:          InboundState,
   inbound_attempts: usize,

   outbound:          OutboundState,
   outbound_attempts: usize,
   outbound_packets:  ringbuf::LocalRb<ringbuf_storage::Array<Packet, PACKET_BUFFER_SIZE>>,
}

#[expect(clippy::large_enum_variant)]
pub enum Handler {
   Enabled(EnabledHandler),
   Disabled,
}

impl Handler {
   fn new(peer_id: p2p::PeerId) -> Self {
      Handler::Enabled(EnabledHandler {
         peer_id,

         inbound: InboundState::Disconnected,
         inbound_attempts: 0,

         outbound: OutboundState::Disconnected,
         outbound_attempts: 0,
         outbound_packets: ringbuf::LocalRb::default(),
      })
   }
}

impl p2p_swarm::ConnectionHandler for Handler {
   type FromBehaviour = Packet;

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

   fn on_behaviour_event(&mut self, packet: Self::FromBehaviour) {
      let Handler::Enabled(ref mut handler) = *self else {
         return;
      };

      if let Err(packet) = handler.outbound_packets.try_push(packet) {
         tracing::warn!(peer_id = %handler.peer_id, "Packet buffer full, dropping oldest packet");
         handler.outbound_packets.try_pop().expect("buffer is full");
         handler
            .outbound_packets
            .try_push(packet)
            .expect("just popped");
      }
   }

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
               tracing::warn!(peer_id = %handler.peer_id, "Maximum inbound substream attempts exceeded");
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
               tracing::warn!(peer_id = %handler.peer_id, "Maximum outbound substream attempts exceeded");
               *self = Handler::Disabled;
            }
         },
         _ => {},
      }
   }

   #[expect(clippy::cognitive_complexity)]
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
               if !handler.outbound_packets.is_empty() {
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
               if let Some(packet) = handler.outbound_packets.try_pop() {
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
                           tracing::debug!(%error, "Failed to send packet on outbound stream");
                           handler.outbound = OutboundState::Disconnected;
                        },
                     }
                  },
                  Ready(Err(error)) => {
                     tracing::debug!(%error, "Failed to send packet on outbound stream");
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
                     tracing::debug!(%error, "Failed to flush outbound stream");
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
                     tracing::debug!(%error, "Failed to read from inbound stream");
                     handler.inbound = InboundState::Closing(substream);
                     continue;
                  },
                  Ready(None) => {
                     tracing::debug!(peer_id = %handler.peer_id, "Inbound stream closed by remote");
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
                     tracing::debug!(%error, "Failed to close inbound stream");
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

pub trait Policy = FnMut(&p2p::PeerId) -> bool + 'static;

pub struct Behaviour<P: Policy> {
   queued_events: VecDeque<p2p_swarm::ToSwarm<Event, Packet>>,

   inbound_policy: P,

   connected: rustc_hash::FxHashSet<p2p::PeerId>,
   pending: rustc_hash::FxHashMap<
      p2p::PeerId,
      ringbuf::LocalRb<ringbuf_storage::Array<Packet, PACKET_BUFFER_SIZE>>,
   >,
}

impl<P: Policy> Behaviour<P> {
   pub fn new(inbound_policy: P) -> Self {
      Self {
         queued_events: VecDeque::new(),

         inbound_policy,

         connected: rustc_hash::FxHashSet::default(),
         pending: rustc_hash::FxHashMap::default(),
      }
   }

   pub fn send(&mut self, peer_id: &p2p::PeerId, packet: Packet) {
      let true = self.connected.contains(peer_id) else {
         self.queued_events.push_back(p2p_swarm::ToSwarm::Dial {
            opts: p2p_swarm_dial_opts::DialOpts::peer_id(*peer_id).build(),
         });

         let buffer = self.pending.entry(*peer_id).or_default();
         if let Err(packet) = buffer.try_push(packet) {
            tracing::warn!(%peer_id, "Packet buffer full, dropping oldest packet");
            buffer.try_pop().expect("buffer is full");
            buffer.try_push(packet).expect("just popped");
         }

         return;
      };

      self
         .queued_events
         .push_back(p2p_swarm::ToSwarm::NotifyHandler {
            peer_id: *peer_id,
            handler: p2p_swarm::NotifyHandler::Any,
            event:   packet,
         });
   }
}

impl<P: Policy> p2p_swarm::NetworkBehaviour for Behaviour<P> {
   type ConnectionHandler = Handler;

   type ToSwarm = Event;

   fn handle_established_inbound_connection(
      &mut self,
      _connection_id: p2p_swarm::ConnectionId,
      peer_id: p2p::PeerId,
      _local_addr: &p2p::Multiaddr,
      _remote_addr: &p2p::Multiaddr,
   ) -> Result<Self::ConnectionHandler, p2p_swarm::ConnectionDenied> {
      if !(self.inbound_policy)(&peer_id) {
         return Ok(Handler::Disabled);
      }

      self.connected.insert(peer_id);
      Ok(Handler::new(peer_id))
   }

   fn handle_established_outbound_connection(
      &mut self,
      _connection_id: p2p_swarm::ConnectionId,
      peer_id: p2p::PeerId,
      _addr: &p2p::Multiaddr,
      _role_override: p2p_core::Endpoint,
      _port_use: p2p_core_transport::PortUse,
   ) -> Result<Self::ConnectionHandler, p2p_swarm::ConnectionDenied> {
      self.connected.insert(peer_id);
      Ok(Handler::new(peer_id))
   }

   fn on_swarm_event(&mut self, event: p2p_swarm::FromSwarm) {
      match event {
         p2p_swarm::FromSwarm::ConnectionClosed(p2p_swarm::ConnectionClosed {
            peer_id,
            remaining_established: 0,
            ..
         }) => {
            self.connected.remove(&peer_id);
         },
         p2p_swarm::FromSwarm::DialFailure(p2p_swarm::DialFailure {
            peer_id: Some(peer_id),
            error: &p2p_swarm::DialError::NoAddresses,
            ..
         }) => {
            self
               .queued_events
               .push_back(p2p_swarm::ToSwarm::GenerateEvent(Event::DiscoverPeer(
                  peer_id,
               )));
         },
         _ => {},
      }
   }

   fn on_connection_handler_event(
      &mut self,
      peer_id: p2p::PeerId,
      _connection_id: p2p_swarm::ConnectionId,
      packet: p2p_swarm::THandlerOutEvent<Self>,
   ) {
      self
         .queued_events
         .push_back(p2p_swarm::ToSwarm::GenerateEvent(Event::Packet(peer_id, packet)));
   }

   fn poll(
      &mut self,
      _context: &mut task::Context<'_>,
   ) -> task::Poll<p2p_swarm::ToSwarm<Self::ToSwarm, p2p_swarm::THandlerInEvent<Self>>> {
      self.pending.retain(|&peer_id, buffer| {
         if !self.connected.contains(&peer_id) {
            return true;
         }

         while let Some(packet) = buffer.try_pop() {
            self
               .queued_events
               .push_back(p2p_swarm::ToSwarm::NotifyHandler {
                  peer_id,
                  handler: p2p_swarm::NotifyHandler::Any,
                  event: packet,
               });
         }

         false
      });

      match self.queued_events.pop_front() {
         Some(event) => task::Poll::Ready(event),
         None => task::Poll::Pending,
      }
   }
}
