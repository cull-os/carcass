#![feature(trait_alias, stmt_expr_attributes)]

use std::{
   net,
   time,
};

use cyn::ResultExt as _;
use derive_more::{
   Deref,
   DerefMut,
};
use indexmap::IndexMap;
use libp2p::{
   self as p2p,
   core::transport as p2p_transport,
   dcutr as p2p_dcutr,
   futures::StreamExt as _,
   identify as p2p_identify,
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
use tokio::select;

pub mod address;

pub mod config;
pub use config::Config;

mod interface;
pub use interface::{
   Interface,
   MTU,
};

pub mod ip;

const RELAY_PEER_TARGET: usize = 4;
const RELAY_ADDRS_PER_PEER: usize = 4;

fn destination_of(packet: &[u8]) -> Option<net::Ipv6Addr> {
   if packet.first()? >> 4_usize != 6 {
      return None;
   }

   Some(net::Ipv6Addr::from(
      <[u8; _]>::try_from(packet.get(24..40)?).expect("size matches"),
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
}

#[derive(Deref, DerefMut)]
struct Program<P: ip::Policy> {
   local: config::LocalPeer,

   tun_buffer:    [u8; MTU as usize],
   tun_interface: Interface,

   network_state:   netmon::State,
   network_monitor: n0_watcher::Direct<netmon::State>,

   address_map: address::Map,

   relays: IndexMap<p2p::PeerId, Relay, rustc_hash::FxBuildHasher>,

   #[deref]
   #[deref_mut]
   swarm: p2p::Swarm<Behaviour<P>>,
}

#[expect(clippy::cognitive_complexity)]
async fn create(config: Config) -> cyn::Result<Program<impl ip::Policy>> {
   let local = config.local()?;

   let mut address_map = address::Map::new(local.id);

   tracing::info!(
      id = %local.id,
      prefix = %net::Ipv6Addr::from(
         address_map
            .prefix_of(local.id)
            .expect("local is always in map")
      ),
      "Local peer added",
   );

   for peer in &config.peers {
      let (&config::Peer::Remote { id } | &config::Peer::RemoteControl { id, .. }) = peer else {
         continue;
      };

      match address_map.prefix_of(id) {
         None => {
            tracing::error!(%id, "Peer has a prefix collision, skipping");
         },
         Some(prefix) => {
            tracing::info!(
               %id,
               prefix = %net::Ipv6Addr::from(prefix),
               "Peer added",
            );
         },
      }
   }

   let mut network_monitor = netmon::Monitor::new()
      .await
      .chain_err("failed to create network monitor")?
      .interface_state();

   Ok(Program {
      local: local.clone(),

      tun_buffer: [0; _],
      tun_interface: Interface::create(
         local.interface.as_deref(),
         address_map
            .prefix_of(local.id)
            .expect("local is always in map"),
      )?,

      network_state: network_monitor.get(),
      network_monitor,

      address_map,

      relays: IndexMap::default(),

      swarm: p2p::SwarmBuilder::with_existing_identity(local.keypair.clone().into())
         .with_tokio()
         .with_tcp(
            p2p_tcp::Config::default(),
            p2p_noise::Config::new,
            p2p_yamux::Config::default,
         )
         .chain_err("failed to create tcp transport layer")?
         .with_quic()
         .with_dns()
         .chain_err("failed to create dns resolver")?
         .with_websocket(p2p_noise::Config::new, p2p_yamux::Config::default)
         .await
         .chain_err("failed to create websocket transport")?
         .with_relay_client(p2p_noise::Config::new, p2p_yamux::Config::default)
         .chain_err("failed to create relay client transport")?
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

                  // Add bootstrap peers to Kademlia DHT for peer discovery.
                  for peer in &config.peers {
                     let &config::Peer::Bootstrap(ref address) = peer else {
                        continue;
                     };

                     let Some(peer_id) = address.iter().find_map(|protocol| {
                        let p2p_multiaddr::Protocol::P2p(peer_id) = protocol else {
                           return None;
                        };

                        Some(peer_id)
                     }) else {
                        tracing::error!(%address, "Bootstrap address has no peer ID, skipping");
                        continue;
                     };

                     kad.add_address(&peer_id, address.clone());
                  }

                  kad
               },

               ip: ip::Behaviour::new({
                  let peer_ids = config
                     .peers
                     .iter()
                     .filter_map(|peer| {
                        match peer {
                           &config::Peer::Remote { id }
                           | &config::Peer::RemoteControl { id, .. } => Some(id),
                           _ => None,
                        }
                     })
                     .collect::<rustc_hash::FxHashSet<_>>();

                  move |peer_id| peer_ids.contains(peer_id)
               }),
            }
         })
         .chain_err("failed to create swarm")?
         .build(),
   })
}

impl<P: ip::Policy> Program<P> {
   fn recover(&mut self) {
      for address in self.external_addresses().cloned().collect::<Vec<_>>() {
         self.remove_external_address(&address);
      }

      if let Err(error) = self.behaviour_mut().kad.bootstrap() {
         tracing::warn!(%error, "Failed to bootstrap kademlia");
      }
   }

   fn active_relays(&self) -> impl Iterator<Item = &p2p::PeerId> {
      self
         .relays
         .iter()
         .filter(|&(_, relay)| {
            relay
               .addresses
               .iter()
               .any(|&(listener_id, _)| listener_id.is_some())
         })
         .map(|(peer_id, _)| peer_id)
   }

   fn fill_relays(&mut self) {
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

         if relay
            .addresses
            .iter()
            .any(|&(listener_id, _)| listener_id.is_some())
         {
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
               tracing::info!(%address, %listener_id, "Listening via relay");
               addresses_listened += 1;
               listener_id_opt.replace(listener_id);
            }
         }

         if addresses_listened > 0 {
            peers_filled += 1;
         }
      }
   }
}

#[expect(clippy::cognitive_complexity, reason = "event loop")]
pub async fn run(config: Config) -> cyn::Result<()> {
   use BehaviourEvent as Be;
   use p2p_swarm::SwarmEvent as Se;

   let mut program = create(config).await?;

   for address in &program.local.listen {
      program
         .swarm
         .listen_on(address.clone())
         .chain_err_with(|| format!("failed to listen on address {address}"))?;
   }

   program.recover();

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
            match swarm_event {
               Se::NewListenAddr { address, .. } => {
                  tracing::info!(%address, "Listening on address");
               },

               Se::OutgoingConnectionError { peer_id: Some(peer_id), error: p2p_swarm::DialError::NoAddresses, .. } => {
                  tracing::info!(%peer_id, "Dial failed with no addresses, discovering via DHT");
                  program.behaviour_mut().kad.get_closest_peers(peer_id);
               },

               Se::Behaviour(Be::Ip(packet)) => {
                  tracing::trace!(?packet, "Got packet");

                  if let Err(error) = program.tun_interface.send(&packet).await {
                     tracing::error!(%error, "Failed to write packet to TUN interface");
                  }
               },

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

               other => tracing::debug!(?other, "Other swarm event"),
            }
         },

         tun_result = program.tun_interface.recv(&mut program.tun_buffer) => {
            let Ok(packet_len) = tun_result.inspect_err(|error| {
               tracing::error!(%error, "Failed to read from TUN interface");
            }) else {
               continue;
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

            // Loopback: write self-addressed packets back to TUN.
            if peer_id == program.local.id {
               if let Err(error) = program.tun_interface.send(packet).await {
                  tracing::error!(%error, "Failed to write loopback packet to TUN interface");
               }
               continue;
            }

            // Send packet to peer
            let packet = ip::Packet(bytes::Bytes::copy_from_slice(packet));
            program.behaviour_mut().ip.send(&peer_id, packet);
         },
      }
   }
}
