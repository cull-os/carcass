use std::str::FromStr as _;

use cyn::ResultExt as _;
use libp2p::{
   self as p2p,
   identity::{
      self as p2p_id,
      ed25519,
   },
};

mod keypair {
   use serde::{
      Deserialize as _,
      de,
   };

   use super::*;

   pub fn serialize<S: serde::Serializer>(
      keypair: &ed25519::Keypair,
      serializer: S,
   ) -> Result<S::Ok, S::Error> {
      let encoded = multibase::encode(multibase::Base::Base58Btc, keypair.to_bytes());

      serializer.serialize_str(&encoded)
   }

   pub fn deserialize<'de, D: serde::Deserializer<'de>>(
      deserializer: D,
   ) -> Result<ed25519::Keypair, D::Error> {
      let string = String::deserialize(deserializer)?;
      let (_, mut decoded) = multibase::decode(&string).map_err(de::Error::custom)?;

      ed25519::Keypair::try_from_bytes(&mut decoded).map_err(de::Error::custom)
   }
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
   pub name: Option<String>,

   #[serde(rename = "key")]
   pub id:      p2p::PeerId,
   #[serde(rename = "private-key", with = "keypair")]
   pub keypair: ed25519::Keypair,

   pub interface: String,

   pub peers: Vec<Peer>,

   pub bootstrap: Vec<p2p::Multiaddr>,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Peer {
   pub name: Option<String>,

   #[serde(rename = "key")]
   pub id: p2p::PeerId,
}

impl Config {
   pub fn generate() -> cyn::Result<Self> {
      let keypair = ed25519::Keypair::generate();
      let id = p2p::PeerId::from_public_key(&p2p_id::PublicKey::from(keypair.public()));

      let config = Self {
         name: Some(
            heck::AsKebabCase(
               hostname::get()
                  .chain_err("failed to get hostname")?
                  .to_string_lossy(),
            )
            .to_string(),
         ),

         id,
         keypair,

         interface: "con".to_owned(),

         peers: Vec::new(),

         #[rustfmt::skip]
         bootstrap: [
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
         .map(|multiaddr| p2p::Multiaddr::from_str(multiaddr).expect("literals are valid"))
         .collect(),
      };

      if let Some(ref name) = config.name {
         tracing::info!("Generated node name '{name}' from hostname.");
      }

      tracing::info!("Generated node id '{id}'.", id = config.id);
      tracing::info!(
         "Using interface '{interface}'.",
         interface = config.interface,
      );
      tracing::info!("Using {n} bootstrap peers.", n = config.bootstrap.len());

      Ok(config)
   }
}
