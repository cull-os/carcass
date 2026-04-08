use std::{
   collections::hash_map,
   net,
   ops::Range,
};

use derive_more::{
   Deref,
   DerefMut,
};
use libp2p as p2p;
use rustc_hash::FxHashMap;
use sha2::Digest as _;

pub const VPN_PREFIX: [u8; 2] = [0xFD, 0x67];
pub const VPN_PREFIX_RANGE: Range<usize> = 0..2;
pub const HOST_PREFIX_RANGE: Range<usize> = VPN_PREFIX_RANGE.end..VPN_PREFIX_RANGE.end + 8;
pub const HOST_SUBNET_RANGE: Range<usize> = HOST_PREFIX_RANGE.end..HOST_PREFIX_RANGE.end + 6;

#[derive(Deref, DerefMut, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Prefix([u8; HOST_PREFIX_RANGE.end]);

impl From<net::Ipv6Addr> for Prefix {
   fn from(addr: net::Ipv6Addr) -> Self {
      let mut octets = [0; _];
      octets.copy_from_slice(&addr.octets()[..HOST_PREFIX_RANGE.end]);
      Self(octets)
   }
}

impl From<Prefix> for net::Ipv6Addr {
   fn from(prefix: Prefix) -> Self {
      let mut octets = [0; _];
      octets[..prefix.len()].copy_from_slice(&*prefix);
      Self::from(octets)
   }
}

pub struct Map {
   peer_to_prefix: FxHashMap<p2p::PeerId, Prefix>,
   prefix_to_peer: FxHashMap<Prefix, p2p::PeerId>,
}

impl Map {
   #[must_use]
   pub fn new(self_id: p2p::PeerId) -> Self {
      let mut map = Self {
         peer_to_prefix: FxHashMap::default(),
         prefix_to_peer: FxHashMap::default(),
      };

      map.add(self_id);
      map
   }

   pub fn add(&mut self, peer_id: p2p::PeerId) -> Option<Prefix> {
      let mut prefix = Prefix([0; _]);
      prefix[VPN_PREFIX_RANGE].copy_from_slice(&VPN_PREFIX);

      let hash = sha2::Sha256::digest(peer_id.to_bytes());
      prefix[HOST_PREFIX_RANGE].copy_from_slice(&hash[..HOST_PREFIX_RANGE.len()]);

      let hash_map::Entry::Vacant(entry) = self.prefix_to_peer.entry(prefix) else {
         return None;
      };

      entry.insert(peer_id);
      self.peer_to_prefix.insert(peer_id, prefix);

      Some(prefix)
   }

   pub fn remove(&mut self, peer_id: &p2p::PeerId) {
      let Some(prefix) = self.peer_to_prefix.remove(peer_id) else {
         return;
      };
      self.prefix_to_peer.remove(&prefix);
   }

   #[must_use]
   pub fn prefix_of(&self, peer_id: &p2p::PeerId) -> Option<Prefix> {
      self.peer_to_prefix.get(peer_id).copied()
   }

   #[must_use]
   pub fn peer_of(&self, prefix: &Prefix) -> Option<p2p::PeerId> {
      self.prefix_to_peer.get(prefix).copied()
   }
}

#[cfg(test)]
mod tests {
   use p2p::identity::{
      self,
      ed25519,
   };
   use proptest::prelude::*;

   use super::*;

   fn peer_id_strategy() -> impl Strategy<Value = p2p::PeerId> {
      any::<[u8; 32]>().prop_map(|bytes| {
         p2p::PeerId::from_public_key(&identity::PublicKey::from(
            ed25519::Keypair::from(
               ed25519::SecretKey::try_from_bytes(bytes).expect("32 bytes is valid ed25519"),
            )
            .public(),
         ))
      })
   }

   proptest! {
      #[test]
      fn prefix_starts_with_fd67(id in peer_id_strategy()) {
         let map = Map::new(id);
         let prefix = map.prefix_of(&id).expect("self always succeeds");

         prop_assert!(prefix.starts_with(&VPN_PREFIX));
      }

      #[test]
      fn prefix_deterministic(id in peer_id_strategy()) {
         let map1 = Map::new(id);
         let map2 = Map::new(id);

         prop_assert_eq!(
            map1.prefix_of(&id).expect("no collision"),
            map2.prefix_of(&id).expect("no collision"),
         );
      }

      #[test]
      fn map_roundtrip(self_id in peer_id_strategy(), peer_id in peer_id_strategy()) {
         let mut map = Map::new(self_id);

         if let Some(prefix) = map.add(peer_id) {
            prop_assert_eq!(map.peer_of(&prefix), Some(peer_id));
         }
      }

      #[test]
      fn prefix_from_ipv6_roundtrip(id in peer_id_strategy()) {
         let map = Map::new(id);
         let prefix = map.prefix_of(&id).expect("self always succeeds");

         prop_assert_eq!(Prefix::from(net::Ipv6Addr::from(prefix)), prefix);
      }
   }
}
