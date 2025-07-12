use libp2p::{
   dcutr as p2p_dcutr,
   gossipsub as p2p_gossipsub,
   identify as p2p_identify,
   kad::{
      self as p2p_kad,
      store as p2p_kad_store,
   },
   relay::{
      self as p2p_relay,
      client as p2p_relay_client,
   },
   swarm as p2p_swarm,
};

#[derive(p2p_swarm::NetworkBehaviour)]
pub struct Behaviour {
   relay_client: p2p_relay_client::Behaviour,
   dcutr:        p2p_dcutr::Behaviour,
   kad:          p2p_kad::Behaviour<p2p_kad_store::MemoryStore>,
   identify:     p2p_identify::Behaviour,
   gossipsub:    p2p_gossipsub::Behaviour,
}
