use std::{
   iter,
   str::FromStr as _,
};

use libp2p::{
   self as p2p,
   identity::{
      self as p2p_id,
      ed25519,
   },
};

#[derive(Clone)]
pub struct Keypair(pub ed25519::Keypair);

impl Keypair {
   const SERIALIZE_PREFIX: &str = "route67-keypair:";

   #[must_use]
   pub fn id(&self) -> p2p::PeerId {
      p2p::PeerId::from_public_key(&p2p_id::PublicKey::from(self.0.public()))
   }
}

impl serde::Serialize for Keypair {
   fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
      let encoded = multibase::encode(multibase::Base::Base58Btc, self.0.to_bytes());
      serializer.serialize_str(&format!(
         "{prefix}{encoded}",
         prefix = Self::SERIALIZE_PREFIX,
      ))
   }
}

impl<'de> serde::Deserialize<'de> for Keypair {
   fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
      use serde::de::Error as _;

      let string = String::deserialize(deserializer)?;
      let string = string
         .strip_prefix(Self::SERIALIZE_PREFIX)
         .ok_or_else(|| D::Error::custom("missing route67-keypair: prefix"))?;

      let (_, mut decoded) = multibase::decode(string).map_err(D::Error::custom)?;

      ed25519::Keypair::try_from_bytes(&mut decoded)
         .map(Keypair)
         .map_err(D::Error::custom)
   }
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct LocalPeer {
   pub keypair:   Keypair,
   pub interface: Option<String>,
   pub listen:    Vec<p2p::Multiaddr>,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Peer {
   Remote { id: p2p::PeerId },
   RemoteControl { keypair: Keypair },
   Local(LocalPeer),
   Bootstrap(p2p::Multiaddr),
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
   pub peers: Vec<Peer>,
}

impl Config {
   pub fn local(&self) -> cyn::Result<&LocalPeer> {
      let mut locals = self.peers.iter().filter_map(|peer| {
         match peer {
            &Peer::Local(ref local) => Some(local),
            _ => None,
         }
      });

      match (locals.next(), locals.next()) {
         (Some(local), None) => Ok(local),
         (None, None) => cyn::bail!("no local peer in config"),
         (Some(_), Some(_)) => cyn::bail!("more than one local peer in config"),
         _ => unreachable!(),
      }
   }

   #[must_use]
   pub fn generate() -> Self {
      Self {
         peers: iter::once(Peer::Local(LocalPeer {
            keypair:   Keypair(ed25519::Keypair::generate()),
            interface: None,
            listen:    [
               "/ip4/0.0.0.0/tcp/0",
               "/ip6/::/tcp/0",
               "/ip4/0.0.0.0/udp/0/quic-v1",
               "/ip6/::/udp/0/quic-v1",
            ]
            .iter()
            .map(|addr| p2p::Multiaddr::from_str(addr).expect("literals are valid"))
            .collect(),
         }))
         .chain(
            #[rustfmt::skip]
            [
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
            .map(|addr| {
               Peer::Bootstrap(p2p::Multiaddr::from_str(addr).expect("literals are valid"))
            }),
         )
         .collect(),
      }
   }
}
