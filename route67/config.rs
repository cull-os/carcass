use std::str::FromStr as _;

use libp2p::{
   self as p2p,
   identity::{
      self as p2p_id,
      ed25519,
   },
};

mod keypair {
   use libp2p::identity::ed25519;

   const PREFIX: &str = "route67private_";

   pub fn serialize<S: serde::Serializer>(
      keypair: &ed25519::Keypair,
      serializer: S,
   ) -> Result<S::Ok, S::Error> {
      let encoded = multibase::encode(multibase::Base::Base58Btc, keypair.to_bytes());
      serializer.serialize_str(&format!("{PREFIX}{encoded}"))
   }

   pub fn deserialize<'de, D: serde::Deserializer<'de>>(
      deserializer: D,
   ) -> Result<ed25519::Keypair, D::Error> {
      use serde::de::{
         Deserialize as _,
         Error as _,
      };

      let string = String::deserialize(deserializer)?;
      let stripped = string
         .strip_prefix(PREFIX)
         .ok_or_else(|| D::Error::custom(format!("missing {PREFIX} prefix")))?;

      let (_, mut decoded) = multibase::decode(stripped).map_err(D::Error::custom)?;

      ed25519::Keypair::try_from_bytes(&mut decoded).map_err(D::Error::custom)
   }
}

mod peer_id {
   use libp2p as p2p;

   const PREFIX: &str = "route67_";

   pub fn serialize<S: serde::Serializer>(
      id: &p2p::PeerId,
      serializer: S,
   ) -> Result<S::Ok, S::Error> {
      serializer.serialize_str(&format!("{PREFIX}{id}"))
   }

   pub fn deserialize<'de, D: serde::Deserializer<'de>>(
      deserializer: D,
   ) -> Result<p2p::PeerId, D::Error> {
      use serde::de::{
         Deserialize as _,
         Error as _,
      };

      let string = String::deserialize(deserializer)?;
      let stripped = string
         .strip_prefix(PREFIX)
         .ok_or_else(|| D::Error::custom(format!("missing {PREFIX} prefix")))?;

      stripped.parse().map_err(D::Error::custom)
   }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Peer {
   pub address: p2p::Multiaddr,

   #[serde(default, skip_serializing_if = "Vec::is_empty")]
   pub allow: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
   #[serde(with = "peer_id")]
   pub id: p2p::PeerId,

   #[serde(with = "keypair")]
   pub keypair: ed25519::Keypair,

   pub interface: Option<String>,
   pub listen:    Vec<p2p::Multiaddr>,

   #[serde(default, rename = "peer")]
   pub peers: Vec<Peer>,
}

impl Config {
   pub fn validate(&self) -> cyn::Result<()> {
      if self.id != p2p::PeerId::from_public_key(&p2p_id::PublicKey::from(self.keypair.public())) {
         cyn::bail!("peer id does not match keypair");
      }

      Ok(())
   }

   #[must_use]
   pub fn generate() -> Self {
      let keypair = ed25519::Keypair::generate();
      let id = p2p::PeerId::from_public_key(&p2p_id::PublicKey::from(keypair.public()));

      Self {
         id,
         keypair,
         interface: None,
         listen: [
            "/ip4/0.0.0.0/tcp/0",
            "/ip6/::/tcp/0",
            "/ip4/0.0.0.0/udp/0/quic-v1",
            "/ip6/::/udp/0/quic-v1",
         ]
         .iter()
         .map(|addr| p2p::Multiaddr::from_str(addr).expect("literals are valid"))
         .collect(),

         #[rustfmt::skip]
         peers: [
            "/ip4/152.67.75.145/tcp/110/p2p/12D3KooWQWsHPUUeFhe4b6pyCaD1hBoj8j6Z7S7kTznRTh1p1eVt",
            "/ip4/152.67.75.145/udp/110/quic-v1/p2p/12D3KooWQWsHPUUeFhe4b6pyCaD1hBoj8j6Z7S7kTznRTh1p1eVt",
            "/ip4/152.67.75.145/tcp/995/p2p/QmbrAHuh4RYcyN9fWePCZMVmQjbaNXtyvrDCWz4VrchbXh",
            "/ip4/152.67.75.145/udp/995/quic-v1/p2p/QmbrAHuh4RYcyN9fWePCZMVmQjbaNXtyvrDCWz4VrchbXh",
            "/ip4/95.216.8.12/tcp/110/p2p/Qmd7QHZU8UjfYdwmjmq1SBh9pvER9AwHpfwQvnvNo3HBBo",
            "/ip4/95.216.8.12/udp/110/quic-v1/p2p/Qmd7QHZU8UjfYdwmjmq1SBh9pvER9AwHpfwQvnvNo3HBBo",
            "/ip4/95.216.8.12/tcp/995/p2p/QmYs4xNBby2fTs8RnzfXEk161KD4mftBfCiR8yXtgGPj4J",
            "/ip4/95.216.8.12/udp/995/quic-v1/p2p/QmYs4xNBby2fTs8RnzfXEk161KD4mftBfCiR8yXtgGPj4J",
            "/ip4/152.67.73.164/tcp/995/p2p/12D3KooWL84sAtq1QTYwb7gVbhSNX5ZUfVt4kgYKz8pdif1zpGUh",
            "/ip4/152.67.73.164/udp/995/quic-v1/p2p/12D3KooWL84sAtq1QTYwb7gVbhSNX5ZUfVt4kgYKz8pdif1zpGUh",
            "/ip4/37.27.11.202/udp/21/quic-v1/p2p/12D3KooWN31twBvdEcxz2jTv4tBfPe3mkNueBwDJFCN4xn7ZwFbi",
            "/ip4/37.27.11.202/udp/443/quic-v1/p2p/12D3KooWN31twBvdEcxz2jTv4tBfPe3mkNueBwDJFCN4xn7ZwFbi",
            "/ip4/37.27.11.202/udp/500/quic-v1/p2p/12D3KooWN31twBvdEcxz2jTv4tBfPe3mkNueBwDJFCN4xn7ZwFbi",
            "/ip4/37.27.11.202/udp/995/quic-v1/p2p/12D3KooWN31twBvdEcxz2jTv4tBfPe3mkNueBwDJFCN4xn7ZwFbi",
            "/dnsaddr/bootstrap.libp2p.io/p2p/12D3KooWEZXjE41uU4EL2gpkAQeDXYok6wghN7wwNVPF5bwkaNfS",
            "/dnsaddr/bootstrap.libp2p.io/p2p/QmNnooDu7bfjPFoTZYxMNLWUQJyrVwtbZg5gBMjTezGAJN",
            "/dnsaddr/bootstrap.libp2p.io/p2p/QmQCU2EcMqAqQPR2i9bChDtGNJchTbq5TbXJJ16u19uLTa",
            "/dnsaddr/bootstrap.libp2p.io/p2p/QmZa1sAxajnQjVM8WjWXoMbmPd7NsWhfKsPkErzpm9wGkp",
            "/dnsaddr/bootstrap.libp2p.io/p2p/QmbLHAnMoJPWSCR5Zhtx6BHJX9KiKNN6tpvbUcqanj75Nb",
            "/dnsaddr/bootstrap.libp2p.io/p2p/QmcZf59bWwK5XFi76CZX8cbJ4BhTzzA3gU1ZjYZcYW3dwt",
         ]
         .iter()
         .map(|address| {
            Peer {
               address: p2p::Multiaddr::from_str(address).expect("literals are valid"),
               allow:   Vec::new(),
            }
         })
         .collect(),
      }
   }
}
