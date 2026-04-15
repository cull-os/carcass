#![feature(adt_const_params, trait_alias, stmt_expr_attributes)]

use std::{
   cell::RefCell,
   net,
   rc::Rc,
   time,
};

use dup::Dupe as _;
use indexmap::IndexMap;
use libp2p::{
   self as p2p,
   core::transport as p2p_transport,
   dcutr as p2p_dcutr,
   futures::{
      FutureExt as _,
      StreamExt as _,
   },
   identify as p2p_identify,
   identity::{
      self as p2p_id,
      ed25519,
   },
   kad::{
      self as p2p_kad,
      store as p2p_kad_store,
   },
   multiaddr as p2p_multiaddr,
   noise as p2p_noise,
   ping as p2p_ping,
   relay::{
      self as p2p_relay,
      client as p2p_relay_client,
   },
   swarm as p2p_swarm,
   tcp as p2p_tcp,
   upnp::tokio as p2p_upnp,
   yamux as p2p_yamux,
};
use n0_watcher::Watcher as _;
use netwatch::netmon;
use rustc_hash::FxHashMap;
use tokio::{
   io,
   select,
   sync::mpsc,
};

pub mod address;

pub mod dns;

pub mod config;
pub use config::Config;

mod interface;
pub use interface::{
   Interface,
   MTU,
};

pub mod ip;

pub mod socket;

fn source_of(packet: &[u8]) -> Option<net::Ipv6Addr> {
   if packet.first()? >> 4_usize != 6 {
      return None;
   }

   Some(net::Ipv6Addr::from(
      <[u8; _]>::try_from(packet.get(8..8 + 16)?).expect("size was statically checked"),
   ))
}

fn destination_of(packet: &[u8]) -> Option<net::Ipv6Addr> {
   if packet.first()? >> 4_usize != 6 {
      return None;
   }

   Some(net::Ipv6Addr::from(
      <[u8; _]>::try_from(packet.get(24..24 + 16)?).expect("size was statically checked"),
   ))
}

#[derive(p2p_swarm::NetworkBehaviour)]
struct Behaviour<P: ip::Policy> {
   identify:     p2p_identify::Behaviour,
   ping:         p2p_ping::Behaviour,
   upnp:         p2p_upnp::Behaviour,
   relay_server: p2p_relay::Behaviour,
   relay_client: p2p_relay_client::Behaviour,
   dcutr:        p2p_dcutr::Behaviour,
   kad:          p2p_kad::Behaviour<p2p_kad_store::MemoryStore>,
   ip:           ip::Behaviour<P>,
}

struct Relay {
   ping:      time::Duration,
   addresses: Vec<(Option<p2p_transport::ListenerId>, p2p::Multiaddr)>,
}

impl Relay {
   fn new(addresses: impl IntoIterator<Item = p2p::Multiaddr>) -> Self {
      Self {
         ping:      time::Duration::MAX,
         addresses: addresses
            .into_iter()
            .map(|address| (None, address))
            .collect(),
      }
   }

   fn is_active(&self) -> bool {
      self
         .addresses
         .iter()
         .any(|&(listener_id, _)| listener_id.is_some())
   }
}

struct Program<P: ip::Policy> {
   tun_buffer:    [u8; MTU as usize],
   tun_interface: Interface,

   network_state:   netmon::State,
   network_monitor: n0_watcher::Direct<netmon::State>,

   address_map:  address::Map,
   mapped_peers: Rc<RefCell<rustc_hash::FxHashSet<p2p::PeerId>>>,

   relays: IndexMap<p2p::PeerId, Relay, rustc_hash::FxBuildHasher>,

   swarm: p2p::Swarm<Behaviour<P>>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
   #[error("failed to parse config")]
   ParseConfig(#[from] config::Error),

   #[error("failed to create network monitor")]
   CreateNetworkMonitor(#[source] netmon::Error),

   #[error("failed to create tun interface")]
   CreateInterface(#[from] interface::Error),

   #[error("failed to create tcp transport layer")]
   CreateTcpTransport(#[source] p2p_noise::Error),

   #[error("failed to create dns resolver")]
   CreateDnsResolver(#[source] io::Error),

   #[error("failed to create websocket transport")]
   CreateWebsocketTransport(#[source] p2p::WebsocketBuilderError<p2p_noise::Error>),

   #[error("failed to create relay client transport")]
   CreateRelayClientTransport(#[source] p2p_noise::Error),

   #[error("failed to listen on address '{address}'")]
   Listen {
      address: p2p::Multiaddr,
      #[source]
      source:  p2p_transport::TransportError<io::Error>,
   },

   #[error("failed to start dns server")]
   StartDnsServer(#[from] dns::ListenError),
}

#[bon::builder]
async fn create(
   peer_id: p2p::PeerId,
   keypair: ed25519::Keypair,
   interface: Option<&str>,
) -> Result<Program<impl ip::Policy>, Error> {
   let address_map = address::Map::new(peer_id);

   tracing::info!(
      %peer_id,
      prefix = %net::Ipv6Addr::from(
         address_map
            .prefix_of(&peer_id)
            .expect("self must be in map")
      ),
      "Local peer mapped",
   );

   let allowed_peers = Rc::new(RefCell::new(rustc_hash::FxHashSet::default()));

   let mut network_monitor = netmon::Monitor::new()
      .await
      .map_err(Error::CreateNetworkMonitor)?
      .interface_state();

   Ok(Program {
      tun_buffer: [0; _],
      tun_interface: Interface::create(
         interface,
         address_map
            .prefix_of(&peer_id)
            .expect("self must be in map"),
      )
      .await?,

      network_state: network_monitor.get(),
      network_monitor,

      address_map,
      mapped_peers: allowed_peers.dupe(),

      relays: IndexMap::default(),

      swarm: p2p::SwarmBuilder::with_existing_identity(p2p_id::Keypair::from(keypair))
         .with_tokio()
         .with_tcp(
            p2p_tcp::Config::default(),
            p2p_noise::Config::new,
            p2p_yamux::Config::default,
         )
         .map_err(Error::CreateTcpTransport)?
         .with_quic()
         .with_dns()
         .map_err(Error::CreateDnsResolver)?
         .with_websocket(p2p_noise::Config::new, p2p_yamux::Config::default)
         .await
         .map_err(Error::CreateWebsocketTransport)?
         .with_relay_client(p2p_noise::Config::new, p2p_yamux::Config::default)
         .map_err(Error::CreateRelayClientTransport)?
         .with_behaviour(|keypair, relay_client| {
            let peer_id = keypair.public().to_peer_id();

            Behaviour {
               identify: p2p_identify::Behaviour::new(p2p_identify::Config::new(
                  p2p_identify::PROTOCOL_NAME.to_string(),
                  keypair.public(),
               )),

               ping: p2p_ping::Behaviour::default(),

               upnp: p2p_upnp::Behaviour::default(),

               relay_server: p2p_relay::Behaviour::new(peer_id, p2p_relay::Config::default()),
               relay_client,

               dcutr: p2p_dcutr::Behaviour::new(peer_id),

               kad: {
                  let mut kad =
                     p2p_kad::Behaviour::new(peer_id, p2p_kad_store::MemoryStore::new(peer_id));

                  kad.set_mode(Some(p2p_kad::Mode::Client));

                  kad
               },

               ip: ip::Behaviour::new(move |peer_id| allowed_peers.borrow().contains(peer_id)),
            }
         })
         .unwrap_or_else(|infallible| match infallible {})
         .build(),
   })
}

impl<P: ip::Policy> Program<P> {
   fn map_peer(&mut self, peer_id: p2p::PeerId, peer: &config::Peer) -> Result<(), String> {
      let Some(prefix) = self.address_map.map(peer_id) else {
         tracing::error!(%peer_id, "Peer has a prefix collision, could not map");
         return Err("failed to map peer due to prefix collision".to_owned());
      };

      if !peer.allow.is_empty() {
         self.mapped_peers.borrow_mut().insert(peer_id);
      }

      for address in &peer.addresses {
         self.swarm.add_peer_address(peer_id, address.clone());
         self
            .swarm
            .behaviour_mut()
            .kad
            .add_address(&peer_id, address.clone());
      }

      tracing::info!(
         %peer_id,
         prefix = %net::Ipv6Addr::from(prefix),
         "Peer mapped",
      );

      Ok(())
   }

   fn unmap_peer(&mut self, peer_id: p2p::PeerId) {
      self.address_map.unmap(&peer_id);

      self.mapped_peers.borrow_mut().remove(&peer_id);

      let _ = self.swarm.disconnect_peer_id(peer_id);
      self.swarm.behaviour_mut().kad.remove_peer(&peer_id);

      tracing::info!(%peer_id, "Peer unmapped");
   }

   fn recover(&mut self) {
      for address in self.swarm.external_addresses().cloned().collect::<Vec<_>>() {
         self.swarm.remove_external_address(&address);
      }

      if let Err(error) = self.swarm.behaviour_mut().kad.bootstrap() {
         tracing::warn!(%error, "Failed to bootstrap kademlia");
      }
   }

   fn active_relays(&self) -> impl Iterator<Item = &p2p::PeerId> {
      self
         .relays
         .iter()
         .filter(|&(_, relay)| relay.is_active())
         .map(|(peer_id, _)| peer_id)
   }

   fn fill_relays(&mut self) {
      const RELAY_PEER_TARGET: usize = 4;
      const RELAY_ADDRS_PER_PEER: usize = 4;

      fn pick_diverse(
         addresses: &mut [(Option<p2p_transport::ListenerId>, p2p::Multiaddr)],
      ) -> impl Iterator<Item = &mut (Option<p2p_transport::ListenerId>, p2p::Multiaddr)> {
         let mut seen = FxHashMap::default();

         for item in addresses.iter_mut() {
            seen
               .entry(
                  item
                     .1
                     .iter()
                     .map(|protocol| protocol.tag())
                     .collect::<Vec<_>>(),
               )
               .or_insert(item);
         }

         seen.into_values()
      }

      let peers_need = RELAY_PEER_TARGET.saturating_sub(self.active_relays().count());

      let mut peers_filled = 0_usize;
      for (&peer_id, relay) in &mut self.relays {
         if peers_filled >= peers_need {
            break;
         }

         if relay.is_active() {
            continue;
         }

         let mut addresses_listened = 0_usize;
         for &mut (ref mut listener_id_opt, ref address) in pick_diverse(&mut relay.addresses) {
            if addresses_listened >= RELAY_ADDRS_PER_PEER {
               break;
            }

            let address = address
               .clone()
               .with(p2p_multiaddr::Protocol::P2p(peer_id))
               .with(p2p_multiaddr::Protocol::P2pCircuit);

            if let Ok(listener_id) = self.swarm.listen_on(address.clone()) {
               addresses_listened += 1;
               listener_id_opt.replace(listener_id);
            }
         }

         if addresses_listened > 0 {
            peers_filled += 1;
         }
      }
   }

   fn pick_closest_relays(&mut self) {
      let Some((worst_active_peer, worst_active_ping)) = self
         .relays
         .iter()
         .filter(|&(_, relay)| relay.is_active())
         .max_by_key(|&(_, relay)| relay.ping)
         .map(|(peer_id, relay)| (*peer_id, relay.ping))
      else {
         return;
      };

      let Some((best_inactive_peer, best_inactive_ping)) = self
         .relays
         .iter()
         .filter(|&(_, relay)| !relay.is_active())
         .min_by_key(|&(_, relay)| relay.ping)
         .map(|(peer_id, relay)| (peer_id, relay.ping))
      else {
         return;
      };

      if best_inactive_ping >= worst_active_ping / 3 * 2
         || worst_active_ping.saturating_sub(best_inactive_ping) <= time::Duration::from_millis(10)
      {
         return;
      }

      tracing::info!(
         %worst_active_peer,
         ?worst_active_ping,
         %best_inactive_peer,
         ?best_inactive_ping,
         "Evicting slow relay for faster candidate",
      );

      let relay = self
         .relays
         .get_mut(&worst_active_peer)
         .expect("peer was just looked up");
      for &mut (ref mut listener_id, _) in &mut relay.addresses {
         if let Some(id) = listener_id.take() {
            self.swarm.remove_listener(id);
         }
      }

      self.fill_relays();
   }
}

#[bon::builder]
#[expect(clippy::cognitive_complexity)]
pub async fn run(
   config: Config,
   mut requests: mpsc::UnboundedReceiver<socket::Exchange<socket::Request, socket::Response>>,
) -> Result<(), Error> {
   use BehaviourEvent as Be;
   use p2p_swarm::SwarmEvent as Se;

   let mut program = create()
      .peer_id(config.peer_id)
      .keypair(config.keypair)
      .maybe_interface(config.interface.as_deref())
      .call()
      .await?;

   for (&peer_id, peer) in &config.peers {
      let _ = program.map_peer(peer_id, peer);
   }

   for address in &config.listen {
      program.swarm.listen_on(address.clone()).map_err(|source| {
         Error::Listen {
            address: address.clone(),
            source,
         }
      })?;
   }

   program.recover();

   let mut dns_queries = dns::listen().await?;

   loop {
      select! {
         Ok(new_network_state) = program.network_monitor.updated() => {
            let unsuspended = match (new_network_state.last_unsuspend, program.network_state.last_unsuspend) {
               (Some(new), Some(old)) => new > old,
               (Some(_), None) => true,
               _ => false,
            };

            if unsuspended || new_network_state.is_major_change(&program.network_state) {
               tracing::info!("Network change detected, recovering");
               program.recover();
            }

            program.network_state = new_network_state;
         },

         swarm_event = program.swarm.select_next_some() => {
            let mut swarm_pending = Some(swarm_event);
            while let Some(swarm_event) = swarm_pending.take().or_else(|| program.swarm.next().now_or_never().flatten()) {
               match swarm_event {
                  Se::NewListenAddr { address, .. } => {
                     if address.iter().any(|part| matches!(part, p2p_multiaddr::Protocol::P2pCircuit)) {
                        tracing::debug!(%address, "Listening on relay address");
                     } else {
                        tracing::debug!(%address, "Listening on address");
                     }
                  },

                  // RELAYS

                  Se::Behaviour(Be::Identify(p2p_identify::Event::Received {
                     peer_id,
                     info,
                     ..
                  })) if info.protocols.contains(&p2p_relay::HOP_PROTOCOL_NAME) => {
                     let mut relay = Relay::new(info.listen_addrs);
                     if let Some(existing) = program.relays.get(&peer_id) {
                        relay.ping = existing.ping;
                     }

                     program.relays.insert(peer_id, relay);
                     program.relays.sort_by(|_, relay_a, _, relay_b| relay_a.ping.cmp(&relay_b.ping));
                     program.fill_relays();
                  },

                  Se::Behaviour(Be::Ping(p2p_ping::Event {
                     peer,
                     result: Ok(ping),
                     ..
                  })) if let Some(ref mut relay) = program.relays.get_mut(&peer) => {
                     relay.ping = ping;
                     program.relays.sort_by(|_, relay_a, _, relay_b| relay_a.ping.cmp(&relay_b.ping));
                     program.pick_closest_relays();
                  },

                  Se::ListenerClosed { listener_id, addresses, .. } => {
                     let is_relay = addresses
                        .iter()
                        .any(|address| {
                           address
                              .iter()
                              .any(|part| matches!(part, p2p_multiaddr::Protocol::P2pCircuit))
                        });

                     if is_relay {
                        for relay in program.relays.values_mut() {
                           for &mut (ref mut id, _) in &mut relay.addresses {
                              if *id == Some(listener_id) {
                                 *id = None;
                              }
                           }
                        }

                        program.fill_relays();
                     }
                  },

                  // IP

                  Se::Behaviour(Be::Ip(ip::Event::Packet(peer_id, packet))) => {
                     tracing::trace!(%peer_id, ?packet, "Got packet");

                     let Some(source) = source_of(&packet) else {
                        tracing::warn!(?packet, "Dropping inbound packet: could not parse source");
                        continue;
                     };

                     match program.address_map.peer_of(&address::Prefix::from(source)) {
                        Some(expected_peer_id) if expected_peer_id == peer_id => {},
                        expected_peer_id => {
                           tracing::warn!(
                              %source,
                              source_peer_id = %peer_id,
                              source_expected_peer_id = ?expected_peer_id,
                              "Dropping packet with spoofed source",
                           );
                           continue;
                        },
                     }

                     if let Err(error) = program.tun_interface.send(&packet).await {
                        tracing::error!(%error, "Failed to write packet to TUN interface");
                     }
                  },

                  Se::Behaviour(Be::Ip(ip::Event::DiscoverPeer(peer_id))) => {
                     tracing::debug!(%peer_id, "Discovering peer via DHT");
                     program.swarm.behaviour_mut().kad.get_closest_peers(peer_id);
                  },

                  other => tracing::debug!(?other, "Other swarm event"),
               }
            }
         },

         tun_result = program.tun_interface.recv(&mut program.tun_buffer) => {
            let mut tun_pending = Some(tun_result);
            while let Some(tun_result) = tun_pending.take().or_else(|| program.tun_interface.recv(&mut program.tun_buffer).now_or_never()) {
               let packet_len = match tun_result {
                  Ok(packet_len) => packet_len,
                  Err(error) => {
                     tracing::error!(%error, "Failed to read from TUN interface");
                     break;
                  },
               };

               let packet = &program.tun_buffer[..packet_len];

               tracing::trace!(?packet, "Got TUN packet");

               let Some(destination) = destination_of(packet) else {
                  tracing::warn!(?packet, "Ignoring invalid TUN packet, could not determine destination");
                  continue;
               };

               // Silently drop multicast packets.
               if !destination.octets().starts_with(&address::VPN_PREFIX) {
                  continue;
               }

               let Some(peer_id) = program.address_map.peer_of(&address::Prefix::from(destination)) else {
                  tracing::warn!(%destination, "Destination not in peer map, dropping");
                  continue;
               };

               if peer_id == *program.swarm.local_peer_id() {
                  tracing::warn!(%destination, "Dropping self-addressed packet, interface should be configured to route this locally");
                  continue;
               }

               let packet = ip::Packet(bytes::Bytes::copy_from_slice(packet));
               program.swarm.behaviour_mut().ip.send(&peer_id, packet);
            }
         },

         Some((request, response)) = requests.recv() => {
            let request = request.await.expect("sender must not be dropped before sending");

            response
               .send(program.handle_request(request))
               .expect("response receiver must stay alive");
         },

         Some(query) = dns_queries.recv() => {
            match query {
               dns::Query::AddressForPeerId { peer_id, sender } => {
                  let address = program.address_map.prefix_of(&peer_id).map(|prefix| {
                     let mut octets = [0; _];
                     octets[..address::HOST_PREFIX_RANGE.end].copy_from_slice(&*prefix);
                     *octets.last_mut().expect("address array must not be empty") = 1;
                     net::Ipv6Addr::from(octets)
                  });

                  sender
                     .send(address)
                     .expect("response receiver must stay alive");
               },
               dns::Query::PeerIdForAddress { address, sender } => {
                  let peer_id = program.address_map.peer_of(&address::Prefix::from(address));

                  sender
                     .send(peer_id)
                     .expect("response receiver must stay alive");
               },
            }
         },
      }
   }
}
