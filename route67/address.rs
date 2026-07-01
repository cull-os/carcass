use std::{
   net,
   ops::Range,
};

use derive_more::{
   Deref,
   DerefMut,
};
use dup::Dupe;
use libp2p as p2p;
use rustc_hash::FxHashMap;
use sha2::Digest as _;

use crate::config;

pub const VPN_PREFIX: [u8; 2] = [0xFD, 0x67];
pub const VPN_PREFIX_RANGE: Range<usize> = 0..2;
pub const HOST_PREFIX_RANGE: Range<usize> = VPN_PREFIX_RANGE.end..VPN_PREFIX_RANGE.end + 8;
pub const HOST_SUBNET_RANGE: Range<usize> = HOST_PREFIX_RANGE.end..HOST_PREFIX_RANGE.end + 6;

#[derive(Deref, DerefMut, Debug, Clone, Copy, Dupe, PartialEq, Eq, Hash)]
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

impl Prefix {
   pub const LOCAL: Self = {
      let mut octets = [0; HOST_PREFIX_RANGE.end];
      octets[VPN_PREFIX_RANGE.start..VPN_PREFIX_RANGE.end].copy_from_slice(&VPN_PREFIX);
      Self(octets)
   };

   #[must_use]
   pub fn host_addr(self) -> net::Ipv6Addr {
      let mut octets = [0; _];
      octets[..self.len()].copy_from_slice(&*self);
      *octets.last_mut().expect("address array must not be empty") = 1;
      net::Ipv6Addr::from(octets)
   }
}

pub struct Map {
   peer_to_prefix: FxHashMap<p2p::PeerId, Prefix>,
   prefix_to_peer: FxHashMap<Prefix, p2p::PeerId>,

   peer_to_aliases: FxHashMap<p2p::PeerId, Vec<config::Alias>>,
   alias_to_peer:   FxHashMap<config::Alias, p2p::PeerId>,
}

#[bon::bon]
impl Map {
   #[must_use]
   #[builder(start_fn = new, finish_fn(name = "self_aliases"))]
   pub fn new_(
      #[builder(start_fn)] self_id: p2p::PeerId,
      #[builder(finish_fn)] self_aliases: &[config::Alias],
   ) -> Self {
      let mut map = Self {
         peer_to_prefix: FxHashMap::default(),
         prefix_to_peer: FxHashMap::default(),

         peer_to_aliases: FxHashMap::default(),
         alias_to_peer:   FxHashMap::default(),
      };

      map.map(self_id).aliases(self_aliases);
      map
   }

   #[builder(finish_fn(name = "aliases"))]
   pub fn map(
      &mut self,
      #[builder(start_fn)] peer_id: p2p::PeerId,
      #[builder(finish_fn)] aliases: &[config::Alias],
   ) -> Option<Prefix> {
      let mut prefix = Prefix([0; _]);
      prefix[VPN_PREFIX_RANGE].copy_from_slice(&VPN_PREFIX);

      let hash = sha2::Sha256::digest(peer_id.to_bytes());
      prefix[HOST_PREFIX_RANGE].copy_from_slice(&hash[..HOST_PREFIX_RANGE.len()]);

      if aliases.iter().any(|alias| {
         self
            .alias_to_peer
            .get(alias)
            .is_some_and(|&owner_id| owner_id != peer_id)
      }) {
         return None;
      }

      if self
         .prefix_to_peer
         .get(&prefix)
         .is_some_and(|&owner_id| owner_id != peer_id)
      {
         return None;
      }

      self.prefix_to_peer.insert(prefix, peer_id);
      self.peer_to_prefix.insert(peer_id, prefix);

      for alias in self.peer_to_aliases.remove(&peer_id).into_iter().flatten() {
         self.alias_to_peer.remove(&alias);
      }

      for alias in aliases {
         self.alias_to_peer.insert(alias.clone(), peer_id);
         self
            .peer_to_aliases
            .entry(peer_id)
            .or_default()
            .push(alias.clone());
      }

      Some(prefix)
   }

   pub fn unmap(&mut self, peer_id: &p2p::PeerId) {
      let Some(prefix) = self.peer_to_prefix.remove(peer_id) else {
         return;
      };
      self.prefix_to_peer.remove(&prefix);

      for alias in self.peer_to_aliases.remove(peer_id).into_iter().flatten() {
         self.alias_to_peer.remove(&alias);
      }
   }

   #[must_use]
   pub fn prefix_of(&self, peer_id: &p2p::PeerId) -> Option<Prefix> {
      self.peer_to_prefix.get(peer_id).copied()
   }

   #[must_use]
   pub fn peer_of(&self, prefix: &Prefix) -> Option<p2p::PeerId> {
      self.prefix_to_peer.get(prefix).copied()
   }

   pub fn aliases_of(&self, peer_id: &p2p::PeerId) -> impl Iterator<Item = &config::Alias> {
      self.peer_to_aliases.get(peer_id).into_iter().flatten()
   }

   #[must_use]
   pub fn peer_of_alias(&self, alias: &config::Alias) -> Option<p2p::PeerId> {
      self.alias_to_peer.get(alias).copied()
   }

   pub fn iter(&self) -> impl Iterator<Item = (p2p::PeerId, Prefix)> + '_ {
      self
         .peer_to_prefix
         .iter()
         .map(|(&peer_id, &prefix)| (peer_id, prefix))
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
      any::<[u8; 32]>().prop_map(|mut bytes| {
         p2p::PeerId::from(identity::PublicKey::from(
            ed25519::Keypair::from(
               ed25519::SecretKey::try_from_bytes(&mut bytes).expect("size was statically checked"),
            )
            .public(),
         ))
      })
   }

   proptest! {
      #[test]
      fn prefix_starts_with_fd67(id in peer_id_strategy()) {
         let map = Map::new(id).self_aliases(&[]);
         let prefix = map.prefix_of(&id).expect("self must be in map");

         prop_assert!(prefix.starts_with(&VPN_PREFIX));
      }

      #[test]
      fn prefix_deterministic(id in peer_id_strategy()) {
         let map1 = Map::new(id).self_aliases(&[]);
         let map2 = Map::new(id).self_aliases(&[]);

         prop_assert_eq!(
            map1.prefix_of(&id).expect("self must be in map"),
            map2.prefix_of(&id).expect("self must be in map"),
         );
      }

      #[test]
      fn map_roundtrip(self_id in peer_id_strategy(), peer_id in peer_id_strategy()) {
         let mut map = Map::new(self_id).self_aliases(&[]);

         if let Some(prefix) = map.map(peer_id).aliases(&[]) {
            prop_assert_eq!(map.peer_of(&prefix), Some(peer_id));
         }
      }

      #[test]
      fn prefix_from_ipv6_roundtrip(id in peer_id_strategy()) {
         let map = Map::new(id).self_aliases(&[]);
         let prefix = map.prefix_of(&id).expect("self must be in map");

         prop_assert_eq!(Prefix::from(net::Ipv6Addr::from(prefix)), prefix);
      }
   }
}
