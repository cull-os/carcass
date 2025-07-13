use libp2p::{
   dcutr as p2p_dcutr,
   identify as p2p_identify,
   identity as p2p_id,
   kad::{
      self as p2p_kad,
      store as p2p_kad_store,
   },
   relay as p2p_relay,
   swarm as p2p_swarm,
};

use crate::Config;

#[derive(p2p_swarm::NetworkBehaviour)]
pub struct Behaviour {
   identify: p2p_identify::Behaviour,
   relay:    p2p_relay::Behaviour,
   dcutr:    p2p_dcutr::Behaviour,
   kad:      p2p_kad::Behaviour<p2p_kad_store::MemoryStore>,
}

impl Behaviour {
   #[must_use]
   pub fn new(keypair: &p2p_id::Keypair, _config: &Config) -> Behaviour {
      let identify = p2p_identify::Behaviour::new(p2p_identify::Config::new(
         "/hyprspace/0.0.1".to_owned(),
         keypair.public(),
      ));

      let relay =
         p2p_relay::Behaviour::new(keypair.public().to_peer_id(), p2p_relay::Config::default());

      let dcutr = p2p_dcutr::Behaviour::new(keypair.public().to_peer_id());

      let kad = p2p_kad::Behaviour::new(
         keypair.public().to_peer_id(),
         p2p_kad_store::MemoryStore::new(keypair.public().to_peer_id()),
      );

      Behaviour {
         identify,
         relay,
         dcutr,
         kad,
      }
   }
}
