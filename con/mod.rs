#![feature(trait_alias)]

use std::{
   net,
   str::FromStr as _,
};

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
   relay as p2p_relay,
   swarm as p2p_swarm,
   tcp as p2p_tcp,
   yamux as p2p_yamux,
};
use tokio::{
   io::{
      AsyncReadExt as _,
      AsyncWriteExt as _,
   },
   select,
};

pub mod address;

pub mod config;
pub use config::Config;

mod interface;
pub use interface::{
   Interface,
   MTU,
};

pub mod ip;

fn ip_of(packet: &[u8]) -> Option<net::IpAddr> {
   Some(match packet.first()? >> 4 {
      4 => {
         net::IpAddr::V4(net::Ipv4Addr::from(
            <[u8; 4]>::try_from(packet.get(16..20)?).expect("size matches"),
         ))
      },
      6 => {
         net::IpAddr::V6(net::Ipv6Addr::from(
            <[u8; 16]>::try_from(packet.get(24..40)?).expect("size matches"),
         ))
      },
      _ => return None,
   })
}

#[derive(p2p_swarm::NetworkBehaviour)]
pub struct Behaviour<P: ip::Policy> {
   pub identify: p2p_identify::Behaviour,
   pub relay:    p2p_relay::Behaviour,
   pub dcutr:    p2p_dcutr::Behaviour,
   pub kad:      p2p_kad::Behaviour<p2p_kad_store::MemoryStore>,
   pub ip:       ip::Behaviour<P>,
}

pub async fn run(config: Config) -> cyn::Result<()> {
   let mut swarm = p2p::SwarmBuilder::with_existing_identity(config.keypair.clone().into())
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

            relay: p2p_relay::Behaviour::new(peer_id, p2p_relay::Config::default()),

            dcutr: p2p_dcutr::Behaviour::new(peer_id),

            kad: {
               let mut kad =
                  p2p_kad::Behaviour::new(peer_id, p2p_kad_store::MemoryStore::new(peer_id));

               // Add bootstrap peers to Kademlia DHT for peer discovery.
               for addr in &config.bootstrap {
                  let Some(peer_id) = addr.iter().find_map(|protocol| {
                     let p2p_multiaddr::Protocol::P2p(peer_id) = protocol else {
                        return None;
                     };

                     Some(peer_id)
                  }) else {
                     tracing::warn!("Bootstrap address '{addr}' has no peer ID, skipping.");
                     continue;
                  };

                  kad.add_address(&peer_id, addr.clone());
               }

               kad
            },

            ip: ip::Behaviour::new({
               let peer_ids = config.peers.iter().map(|peer| peer.id).collect::<Vec<_>>();

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

   for addr in config.listen {
      swarm
         .listen_on(addr)
         .chain_err("failed to listen on local port")?;
   }

   swarm
      .behaviour_mut()
      .kad
      .bootstrap()
      .chain_err("failed to start DHT bootstrap")?;

   let mut tun_buffer = vec![0_u8; MTU as usize];
   let mut tun_interface = Interface::create(
      &config.interface,
      address::generate_v4(&config.id),
      address::generate_v6(&config.id),
   )?;

   let mut address_map = address::Map::new();

   for peer in &config.peers {
      address_map.v4_of(peer.id);
      address_map.v6_of(peer.id);
   }

   loop {
      select! {
         swarm_event = swarm.select_next_some() => {
            match swarm_event {
               p2p_swarm::SwarmEvent::NewListenAddr { address, .. } => {
                  tracing::info!("Listening on {address:?}.");
               },

               p2p_swarm::SwarmEvent::Behaviour(BehaviourEvent::Ip(packet)) => {
                  tracing::trace!("Got packet: {packet:?}");

                  if let Err(error) = tun_interface.write_all(&packet).await {
                     tracing::warn!("Failed to write packet to TUN interface: {error}");
                  }
               },

               other => tracing::debug!("Other swarm event: {other:?}."),
            }
         },

         tun_result = tun_interface.read(&mut tun_buffer) => {
            let Ok(packet_len) = tun_result.inspect_err(|error| {
               tracing::warn!("Failed to read from TUN interface: {error}");
            }) else {
               continue;
            };

            let packet = &tun_buffer[..packet_len];

            tracing::trace!("Got tun packet: {packet:?}");

            let Some(ip) = ip_of(packet) else {
               tracing::warn!("Ignoring invalid tun packet (could not determine ip) {packet:?}");
               continue;
            };

            let peer_id = match ip {
               net::IpAddr::V4(v4) => address_map.peer_of_v4(&v4),
               net::IpAddr::V6(v6) => address_map.peer_of_v6(&v6),
            };

            let Some(peer_id) = peer_id else {
               tracing::warn!("Tried to send packet to ip {ip} not in peer map, dropping.");
               continue;
            };

            // Send packet to peer
            let packet = ip::Packet(bytes::Bytes::copy_from_slice(packet));
            swarm.behaviour_mut().ip.send(&peer_id, packet);
         },
      }
   }
}
