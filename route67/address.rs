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

#[derive(Deref, DerefMut, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Prefix([u8; HOST_PREFIX_RANGE.end / 8]);

impl Prefix {
   #[expect(clippy::cast_possible_truncation, reason = "try_from + expect is not const compatible yet (in Rust 1.96)")]
   pub const BITS: u32 = size_of::<Self>() as u32 * 8;
}

impl From<net::Ipv6Addr> for Prefix {
   fn from(addr: net::Ipv6Addr) -> Self {
      let octets = addr.octets();
      Self(<[u8; _]>::try_from(&octets[..HOST_PREFIX_RANGE.end / 8]).expect("size matches"))
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

      // fd67::
      for (index, bit) in bits([0xFD, 0x67]).enumerate() {
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
