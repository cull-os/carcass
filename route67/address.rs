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

const fn assert_byte_sized(bits: Range<usize>) -> Range<usize> {
   assert!(bits.end.is_multiple_of(8));
   assert!(bits.start.is_multiple_of(8));
   bits
}

pub const VPN_PREFIX: [u8; 2] = [0xFD, 0x67];
pub const VPN_PREFIX_RANGE: Range<usize> = assert_byte_sized(0..16);
pub const HOST_PREFIX_RANGE: Range<usize> =
   assert_byte_sized(VPN_PREFIX_RANGE.end..VPN_PREFIX_RANGE.end + 64);
pub const HOST_SUBNET_RANGE: Range<usize> =
   assert_byte_sized(HOST_PREFIX_RANGE.end..HOST_PREFIX_RANGE.end + 48);

fn bits(bytes: impl IntoIterator<Item = u8>) -> impl Iterator<Item = bool> {
   bytes
      .into_iter()
      .flat_map(|byte| (0_usize..8_usize).map(move |index| byte & (1 << (7_usize - index)) != 0))
}

fn from_bits<const N: usize>(bits: impl IntoIterator<Item = bool>) -> [u8; N] {
   let mut to = [0; N];

   for (index, bit) in bits.into_iter().enumerate() {
      if bit {
         to[index / 8] |= 1 << (7 - (index % 8));
      }
   }

   to
}

fn hash(to: &mut [bool], from: impl IntoIterator<Item = bool>) {
   for (index, from_bit) in from.into_iter().enumerate() {
      to[index % to.len()] ^= from_bit;
   }
}

#[derive(Deref, DerefMut, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Prefix([u8; HOST_PREFIX_RANGE.end / 8]);

impl Prefix {
   #[expect(
      clippy::cast_possible_truncation,
      reason = "try_from + expect is not const compatible yet (in Rust 1.96)"
   )]
   pub const BITS: u32 = size_of::<Self>() as u32 * 8;
}

impl From<net::Ipv6Addr> for Prefix {
   fn from(addr: net::Ipv6Addr) -> Self {
      let mut octets = [0; _];
      octets.copy_from_slice(&addr.octets()[..HOST_PREFIX_RANGE.end / 8]);
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

      map.prefix_of(self_id);
      map
   }

   pub fn prefix_of(&mut self, peer_id: p2p::PeerId) -> Option<Prefix> {
      if let Some(&prefix) = self.peer_to_prefix.get(&peer_id) {
         return Some(prefix);
      }

      let mut prefix = [false; HOST_PREFIX_RANGE.end];

      for (index, bit) in bits(VPN_PREFIX).enumerate() {
         prefix[index] = bit;
      }

      // fd67:dead:beef:cafe:babe::
      hash(
         &mut prefix[HOST_PREFIX_RANGE.clone()],
         bits(peer_id.to_bytes()),
      );

      let prefix = Prefix(from_bits(prefix.iter().copied()));

      let hash_map::Entry::Vacant(entry) = self.prefix_to_peer.entry(prefix) else {
         return None;
      };

      entry.insert(peer_id);
      self.peer_to_prefix.insert(peer_id, prefix);

      Some(prefix)
   }

   #[must_use]
   pub fn peer_of(&self, prefix: &Prefix) -> Option<p2p::PeerId> {
      self.prefix_to_peer.get(prefix).copied()
   }
}

#[cfg(test)]
mod tests {
   use std::iter;

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
         let mut map = Map::new(id);
         let prefix = map.prefix_of(id).expect("self always succeeds");

         prop_assert!(prefix.starts_with(&VPN_PREFIX));
      }

      #[test]
      fn prefix_deterministic(id in peer_id_strategy()) {
         let mut map1 = Map::new(id);
         let mut map2 = Map::new(id);

         prop_assert_eq!(
            map1.prefix_of(id).expect("no collision"),
            map2.prefix_of(id).expect("no collision"),
         );
      }

      #[test]
      fn map_roundtrip(self_id in peer_id_strategy(), peer_id in peer_id_strategy()) {
         let mut map = Map::new(self_id);

         if let Some(prefix) = map.prefix_of(peer_id) {
            prop_assert_eq!(map.peer_of(&prefix), Some(peer_id));
         }
      }

      #[test]
      fn prefix_from_ipv6_roundtrip(id in peer_id_strategy()) {
         let mut map = Map::new(id);
         let prefix = map.prefix_of(id).expect("self always succeeds");

         let v6 = net::Ipv6Addr::from(
            <[u8; _]>::try_from(
               prefix
                  .iter()
                  .copied()
                  .chain(iter::repeat(0))
                  .take(size_of::<net::Ipv6Addr>())
                  .collect::<Vec<_>>(),
            )
            .expect("size matches"),
         );

         prop_assert_eq!(Prefix::from(v6), prefix);
      }
   }
}
