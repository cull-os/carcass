use std::net;

use libp2p as p2p;
use rustc_hash::FxHashMap;

const V4_BASE: [u8; 4] = [100, 64, 1, 2];
const V6_BASE: [u8; 16] = *b"\xFD\0hyprspace\0\0\0\0\0";

/// Generate IPv4 address.
#[must_use]
pub fn generate_v4(peer_id: &p2p::PeerId) -> net::Ipv4Addr {
   let mut addr = V4_BASE;

   for (index, byte) in peer_id.to_bytes().into_iter().enumerate() {
      addr[(index % 2) + 2] ^= byte;
   }

   net::Ipv4Addr::new(addr[0], addr[1], addr[2], addr[3])
}

/// Generate IPv6 address.
#[must_use]
pub fn generate_v6(peer_id: &p2p::PeerId) -> net::Ipv6Addr {
   let mut addr = V6_BASE;

   let mut net_id = [0xDE, 0xAD, 0xBE, 0xEF];
   for (index, byte) in peer_id.to_bytes().into_iter().enumerate() {
      net_id[index % net_id.len()] ^= byte;
   }

   addr[12..].copy_from_slice(&net_id);

   net::Ipv6Addr::from(addr)
}

// /// Generate an IPv6 address for a given name.
// #[must_use]
// #[expect(clippy::cast_possible_truncation)]
// pub fn generate_named_v6(peer_id: &p2p::PeerId, name: &str) -> net::Ipv6Addr
// {    assert!(name.len() < u8::MAX as usize, "name len must fit in a byte",);

//    let mut addr = V6_BASE;

//    let mut net_id = *b"\xDE\xAD\xBE\xEF";
//    for (index, byte) in peer_id.to_bytes().into_iter().enumerate() {
//       net_id[index % net_id.len()] ^= byte;
//    }

//    addr[10..14].copy_from_slice(&net_id);

//    let mut name_id = *b"\xFF\xFE";
//    for (index, &byte) in name.as_bytes().iter().enumerate() {
//       name_id[index % name_id.len()] ^= byte.wrapping_mul(index as u8);
//    }

//    addr[14..].copy_from_slice(&name_id);

//    net::Ipv6Addr::from(addr)
// }

pub struct Map {
   peer_to_v4: FxHashMap<p2p::PeerId, net::Ipv4Addr>,
   peer_to_v6: FxHashMap<p2p::PeerId, net::Ipv6Addr>,
   v4_to_peer: FxHashMap<net::Ipv4Addr, p2p::PeerId>,
   v6_to_peer: FxHashMap<net::Ipv6Addr, p2p::PeerId>,
}

impl Map {
   #[must_use]
   pub fn new() -> Self {
      Self {
         peer_to_v4: FxHashMap::default(),
         peer_to_v6: FxHashMap::default(),
         v4_to_peer: FxHashMap::default(),
         v6_to_peer: FxHashMap::default(),
      }
   }

   // TODO: Make sure these don't collide.

   pub fn v4_of(&mut self, peer_id: p2p::PeerId) -> net::Ipv4Addr {
      *self.peer_to_v4.entry(peer_id).or_insert_with(|| {
         let v4 = generate_v4(&peer_id);
         self.v4_to_peer.insert(v4, peer_id);
         v4
      })
   }

   pub fn v6_of(&mut self, peer_id: p2p::PeerId) -> net::Ipv6Addr {
      *self.peer_to_v6.entry(peer_id).or_insert_with(|| {
         let v6 = generate_v6(&peer_id);
         self.v6_to_peer.insert(v6, peer_id);
         v6
      })
   }

   #[must_use]
   #[expect(clippy::trivially_copy_pass_by_ref)]
   pub fn peer_of_v4(&self, addr: &net::Ipv4Addr) -> Option<p2p::PeerId> {
      self.v4_to_peer.get(addr).copied()
   }

   #[must_use]
   pub fn peer_of_v6(&self, addr: &net::Ipv6Addr) -> Option<p2p::PeerId> {
      self.v6_to_peer.get(addr).copied()
   }
}

#[cfg(test)]
mod tests {
   use super::*;

   /// Helper to create a peer ID from a fixed byte array.
   fn new_peer_id(bytes: &[u8]) -> p2p::PeerId {
      let mut multihash = vec![
         0x00,
         u8::try_from(bytes.len()).expect("bytes len must fit in u8"),
      ];

      multihash.extend(bytes);

      p2p::PeerId::from_bytes(&multihash).expect("bytes must be valid")
   }

   #[test]
   fn map() {
      let mut map = Map::new();

      let peer_id = new_peer_id(&[0x12, 0x34]);

      let v4 = map.v4_of(peer_id);
      let v6 = map.v6_of(peer_id);

      assert_eq!(map.peer_of_v4(&v4), Some(peer_id));
      assert_eq!(map.peer_of_v6(&v6), Some(peer_id));

      assert_eq!(v4, generate_v4(&peer_id));
      assert_eq!(v6, generate_v6(&peer_id));
   }
}
