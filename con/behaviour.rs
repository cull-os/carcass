use std::str::FromStr as _;

use cyn::ResultExt as _;
use libp2p::{
   self as p2p,
   dcutr as p2p_dcutr,
   futures::StreamExt as _,
   identify as p2p_identify,
   identity as p2p_id,
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

use crate::{
   Config,
   address,
   interface,
   vpn,
};

#[derive(p2p_swarm::NetworkBehaviour)]
pub struct Behaviour {
   pub identify: p2p_identify::Behaviour,
   pub relay:    p2p_relay::Behaviour,
   pub dcutr:    p2p_dcutr::Behaviour,
   pub kad:      p2p_kad::Behaviour<p2p_kad_store::MemoryStore>,
   pub vpn:      vpn::Behaviour,
}

impl Behaviour {
   #[must_use]
   pub fn new(keypair: &p2p_id::Keypair, config: &Config) -> Behaviour {
      let peer_id = keypair.public().to_peer_id();

      let identify = p2p_identify::Behaviour::new(p2p_identify::Config::new(
         p2p_identify::PROTOCOL_NAME.to_string(),
         keypair.public(),
      ));

      let relay = p2p_relay::Behaviour::new(peer_id, p2p_relay::Config::default());

      let dcutr = p2p_dcutr::Behaviour::new(peer_id);

      let mut kad = p2p_kad::Behaviour::new(peer_id, p2p_kad_store::MemoryStore::new(peer_id));

      // Add bootstrap peers to Kademlia DHT for peer discovery.
      for addr in &config.bootstrap {
         let Some(peer_id) = addr.iter().find_map(|protocol| {
            let p2p_multiaddr::Protocol::P2p(peer_id) = protocol else {
               return None;
            };

            Some(peer_id)
         }) else {
            continue;
         };

         kad.add_address(&peer_id, addr.clone());
      }

      let vpn = vpn::new();

      Behaviour {
         identify,
         relay,
         dcutr,
         kad,
         vpn,
      }
   }
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
      .with_behaviour(|keypair| Behaviour::new(keypair, &config))
      .unwrap()
      .build();

   swarm
      .listen_on(p2p::Multiaddr::from_str("/ip6/::/tcp/0").expect("literal is valid"))
      .chain_err("failed to listen on local port")?;

   swarm
      .behaviour_mut()
      .kad
      .bootstrap()
      .chain_err("failed to start DHT bootstrap")?;

   let _tun_interface = interface::Interface::create(
      &config.interface,
      address::generate_v4(&config.id),
      address::generate_v6(&config.id),
   )
   .chain_err("failed to create tun interface")?;

   #[expect(clippy::infinite_loop)]
   loop {
      match swarm.select_next_some().await {
         p2p_swarm::SwarmEvent::NewListenAddr { address, .. } => {
            tracing::info!("Listening on {address:?}.");
         },
         p2p_swarm::SwarmEvent::Behaviour(event) => tracing::info!("Behaviour: {event:?}."),
         other => tracing::info!("Other: {other:?}."),
      }
   }
}
