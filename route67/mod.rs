#![feature(trait_alias, stmt_expr_attributes)]

use std::net;

use cyn::ResultExt as _;
use libp2p::{
   self as p2p,
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
   relay as p2p_relay,
   swarm as p2p_swarm,
   tcp as p2p_tcp,
   yamux as p2p_yamux,
};
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
   identify: p2p_identify::Behaviour,
   ping:     p2p_ping::Behaviour,
   relay:    p2p_relay::Behaviour,
   dcutr:    p2p_dcutr::Behaviour,
   kad:      p2p_kad::Behaviour<p2p_kad_store::MemoryStore>,
   ip:       ip::Behaviour<P>,
}

pub async fn run(config: Config) -> cyn::Result<()> {
   let local = config.local()?;

   let mut address_map = address::Map::new(local.id);
   for peer in &config.peers {
      let (&config::Peer::Remote { id } | &config::Peer::RemoteControl { id, .. }) = peer else {
         continue;
      };

      if address_map.prefix_of(id).is_none() {
         tracing::error!("Peer '{id}' has a prefix collision, skipping.");
      }
   }

   let mut tun_buffer = vec![0_u8; MTU as usize];
   let tun_interface = Interface::create(
      local.interface.as_deref(),
      address_map
         .prefix_of(local.id)
         .expect("self is always in map"),
   )?;

   let mut swarm = p2p::SwarmBuilder::with_existing_identity(local.keypair.clone().into())
      .with_tokio()
      .with_tcp(
         p2p_tcp::Config::default(),
         p2p_noise::Config::new,
         p2p_yamux::Config::default,
      )
      .chain_err("failed to create tcp transport layer")?
      .with_quic()
      .with_behaviour(|keypair| {
         let peer_id = keypair.public().to_peer_id();

         Behaviour {
            identify: p2p_identify::Behaviour::new(p2p_identify::Config::new(
               p2p_identify::PROTOCOL_NAME.to_string(),
               keypair.public(),
            )),

            ping: p2p_ping::Behaviour::default(),

            relay: p2p_relay::Behaviour::new(peer_id, p2p_relay::Config::default()),

            dcutr: p2p_dcutr::Behaviour::new(peer_id),

            kad: {
               let mut kad =
                  p2p_kad::Behaviour::new(peer_id, p2p_kad_store::MemoryStore::new(peer_id));

               // Add bootstrap peers to Kademlia DHT for peer discovery.
               for addr in config.peers.iter().filter_map(|peer| {
                  match peer {
                     &config::Peer::Bootstrap(ref addr) => Some(addr),
                     _ => None,
                  }
               }) {
                  let Some(peer_id) = addr.iter().find_map(|protocol| {
                     let p2p_multiaddr::Protocol::P2p(peer_id) = protocol else {
                        return None;
                     };

                     Some(peer_id)
                  }) else {
                     tracing::error!("Bootstrap address '{addr}' has no peer ID, skipping.");
                     continue;
                  };

                  kad.add_address(&peer_id, addr.clone());
               }

               kad
            },

            ip: ip::Behaviour::new({
               let peer_ids = config
                  .peers
                  .iter()
                  .filter_map(|peer| {
                     match peer {
                        &config::Peer::Remote { id } | &config::Peer::RemoteControl { id, .. } => {
                           Some(id)
                        },
                        _ => None,
                     }
                  })
                  .collect::<rustc_hash::FxHashSet<_>>();

               move |peer_id| {
                  if !peer_ids.contains(peer_id) {
                     return Err(p2p_swarm::ConnectionDenied::new(format!(
                        "peer '{peer_id}' is not in the peer list"
                     )));
                  }

                  Ok(())
               }
            }),
         }
      })
      .unwrap()
      .build();

   for addr in &local.listen {
      swarm
         .listen_on(addr.clone())
         .chain_err("failed to listen on local port")?;
   }

   let allowed_ids = config
      .peers
      .iter()
      .filter_map(|peer| {
         match peer {
            &config::Peer::Remote { id } | &config::Peer::RemoteControl { id, .. } => Some(id),
            _ => None,
         }
      })
      .collect::<rustc_hash::FxHashSet<_>>();

   loop {
      select! {
         swarm_event = swarm.select_next_some() => {
            match swarm_event {
               p2p_swarm::SwarmEvent::NewListenAddr { address, .. } => {
                  tracing::info!("Listening on {address:?}.");
               },

               p2p_swarm::SwarmEvent::Behaviour(BehaviourEvent::Ip(packet)) => {
                  tracing::trace!("Got packet: {packet:?}");

                  if let Err(error) = tun_interface.send(&packet).await {
                     tracing::error!("Failed to write packet to TUN interface: {error}");
                  }
               },

               p2p_swarm::SwarmEvent::OutgoingConnectionError { peer_id: Some(peer_id), .. }
                  if allowed_ids.contains(&peer_id) =>
               {
                  tracing::info!("Dial to '{peer_id}' failed, discovering via DHT.");
                  swarm.behaviour_mut().kad.get_closest_peers(peer_id);
               },

               other => tracing::debug!("Other swarm event: {other:?}."),
            }
         },

         tun_result = tun_interface.recv(&mut tun_buffer) => {
            let Ok(packet_len) = tun_result.inspect_err(|error| {
               tracing::error!("Failed to read from TUN interface: {error}");
            }) else {
               continue;
            };

            let packet = &tun_buffer[..packet_len];

            tracing::trace!("Got tun packet: {packet:?}");

            let Some(destination) = destination_of(packet) else {
               tracing::warn!("Ignoring invalid tun packet (could not determine destination) {packet:?}");
               continue;
            };

            // Silently drop multicast packets.
            if !destination.octets().starts_with(&address::VPN_PREFIX) {
               continue;
            }

            let Some(peer_id) = address_map.peer_of(&address::Prefix::from(destination)) else {
               tracing::warn!("Tried to send packet to {destination} not in peer map, dropping.");
               continue;
            };

            // Send packet to peer
            let packet = ip::Packet(bytes::Bytes::copy_from_slice(packet));
            swarm.behaviour_mut().ip.send(&peer_id, packet);
         },
      }
   }
}
