use std::{
   fs,
   io,
   path,
   str::{
      self,
      FromStr as _,
   },
};

use derive_more::{
   Display,
   Into,
};
use hickory_server::proto::{
   self as hickory_proto,
   rr,
};
use indexmap::IndexMap;
use libp2p::{
   self as p2p,
   identity::{
      self as p2p_id,
      ed25519,
   },
};
use serde::de::{
   self as serde_de,
   value as serde_value,
};
use toml::de as toml_de;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FileOrInline<T> {
   File(path::PathBuf),
   #[serde(untagged)]
   Inline(T),
}

impl<T: serde_de::DeserializeOwned + Clone> FileOrInline<T> {
   #[expect(clippy::result_large_err)]
   pub fn content(&self) -> Result<T, Error> {
      use serde::de::IntoDeserializer as _;

      match *self {
         Self::Inline(ref value) => Ok(value.clone()),
         Self::File(ref path) => {
            let string = fs::read_to_string(path).map_err(|source| {
               Error::ReadFile {
                  path: path.clone(),
                  source,
               }
            })?;

            T::deserialize(string.trim().into_deserializer()).map_err(Error::DeserializeFile)
         },
      }
   }
}

#[derive(Debug, Clone, Into)]
pub struct Keypair(ed25519::Keypair);

impl Keypair {
   const PREFIX: &str = "route67private_";
}

impl serde::Serialize for Keypair {
   fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
      serializer.collect_str(&format_args!(
         "{prefix}{encoded}",
         prefix = Self::PREFIX,
         encoded = multibase::encode(multibase::Base::Base58Btc, self.0.to_bytes()),
      ))
   }
}

impl<'de> serde::Deserialize<'de> for Keypair {
   fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
      use serde::de::Error as _;

      let string = String::deserialize(deserializer)?;
      let stripped = string.strip_prefix(Self::PREFIX).ok_or_else(|| {
         D::Error::custom(format_args!(
            "missing '{prefix}' prefix",
            prefix = Self::PREFIX,
         ))
      })?;

      let (_, mut decoded) = multibase::decode(stripped).map_err(D::Error::custom)?;

      ed25519::Keypair::try_from_bytes(&mut decoded)
         .map(Self)
         .map_err(D::Error::custom)
   }
}

#[derive(Debug, Clone, Copy, Into)]
pub struct PeerId(p2p::PeerId);

impl PeerId {
   const PREFIX: &str = "route67_";
}

impl serde::Serialize for PeerId {
   fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
      serializer.collect_str(&format_args!(
         "{prefix}{peer_id}",
         prefix = Self::PREFIX,
         peer_id = self.0,
      ))
   }
}

impl<'de> serde::Deserialize<'de> for PeerId {
   fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
      use serde::de::Error as _;

      let string = String::deserialize(deserializer)?;
      let stripped = string.strip_prefix(Self::PREFIX).ok_or_else(|| {
         D::Error::custom(format_args!(
            "missing '{prefix}' prefix",
            prefix = Self::PREFIX,
         ))
      })?;

      stripped.parse().map(Self).map_err(D::Error::custom)
   }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Display, Into)]
pub struct Alias(rr::Name);

#[derive(Debug, thiserror::Error)]
pub enum AliasError {
   #[error("failed to parse dns name")]
   Parse(#[source] hickory_proto::ProtoError),

   #[error("alias '{name}' must not be fully qualified")]
   FullyQualified { name: rr::Name },
}

impl str::FromStr for Alias {
   type Err = AliasError;

   fn from_str(string: &str) -> Result<Self, Self::Err> {
      let name = rr::Name::from_str(string).map_err(AliasError::Parse)?;

      let false = name.is_fqdn() else {
         return Err(AliasError::FullyQualified { name });
      };

      Ok(Self(name))
   }
}

impl serde::Serialize for Alias {
   fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
      self.0.serialize(serializer)
   }
}

impl<'de> serde::Deserialize<'de> for Alias {
   fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
      use serde::de::Error as _;

      let string = String::deserialize(deserializer)?;
      string.parse().map_err(D::Error::custom)
   }
}

fn is_default<T: Default + Eq>(value: &T) -> bool {
   value == &T::default()
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Peer {
   #[serde(default, skip_serializing_if = "is_default")]
   pub addresses: Vec<p2p::Multiaddr>,

   #[serde(default, skip_serializing_if = "is_default")]
   pub aliases: Vec<Alias>,

   #[serde(default, skip_serializing_if = "is_default")]
   pub allow: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
   #[serde(rename = "id")]
   pub peer_id: PeerId,
   pub keypair: FileOrInline<Keypair>,

   #[serde(default, skip_serializing_if = "is_default")]
   pub interface: Option<String>,
   #[serde(default, skip_serializing_if = "is_default")]
   pub listen:    Vec<p2p::Multiaddr>,

   #[serde(default, skip_serializing_if = "is_default")]
   pub aliases: Vec<Alias>,

   #[serde(default, skip_serializing_if = "is_default")]
   pub zone: Option<FileOrInline<String>>,

   #[serde(default, rename = "peer", skip_serializing_if = "is_default")]
   pub peers: IndexMap<p2p::PeerId, Peer>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
   #[error("{0}")]
   Parse(#[from] toml_de::Error),

   #[error("peer id '{id}' does not match keypair id '{keypair_id}'")]
   PeerIdMismatch {
      id:         p2p::PeerId,
      keypair_id: p2p::PeerId,
   },

   #[error("failed to read file '{path}'", path = .path.display())]
   ReadFile {
      path:   path::PathBuf,
      #[source]
      source: io::Error,
   },

   #[error("failed to deserialize file")]
   DeserializeFile(#[source] serde_value::Error),
}

impl TryFrom<&str> for Config {
   type Error = Error;

   fn try_from(string: &str) -> Result<Self, Error> {
      let config: Self = toml::from_str(string)?;

      let keypair_id = p2p::PeerId::from(p2p_id::PublicKey::from(
         ed25519::Keypair::from(config.keypair.content()?).public(),
      ));

      if p2p::PeerId::from(config.peer_id) != keypair_id {
         return Err(Error::PeerIdMismatch {
            id: p2p::PeerId::from(config.peer_id),
            keypair_id,
         });
      }

      Ok(config)
   }
}

impl Config {
   #[must_use]
   pub fn generate() -> Self {
      #[rustfmt::skip]
      const PEERS: &[(&str, &[&str])] = &[
         ("12D3KooWQWsHPUUeFhe4b6pyCaD1hBoj8j6Z7S7kTznRTh1p1eVt", &[
            "/ip4/152.67.75.145/tcp/110",
            "/ip4/152.67.75.145/udp/110/quic-v1",
         ]),
         ("QmbrAHuh4RYcyN9fWePCZMVmQjbaNXtyvrDCWz4VrchbXh", &[
            "/ip4/152.67.75.145/tcp/995",
            "/ip4/152.67.75.145/udp/995/quic-v1",
         ]),
         ("Qmd7QHZU8UjfYdwmjmq1SBh9pvER9AwHpfwQvnvNo3HBBo", &[
            "/ip4/95.216.8.12/tcp/110",
            "/ip4/95.216.8.12/udp/110/quic-v1",
         ]),
         ("QmYs4xNBby2fTs8RnzfXEk161KD4mftBfCiR8yXtgGPj4J", &[
            "/ip4/95.216.8.12/tcp/995",
            "/ip4/95.216.8.12/udp/995/quic-v1",
         ]),
         ("12D3KooWL84sAtq1QTYwb7gVbhSNX5ZUfVt4kgYKz8pdif1zpGUh", &[
            "/ip4/152.67.73.164/tcp/995",
            "/ip4/152.67.73.164/udp/995/quic-v1",
         ]),
         ("12D3KooWN31twBvdEcxz2jTv4tBfPe3mkNueBwDJFCN4xn7ZwFbi", &[
            "/ip4/37.27.11.202/udp/21/quic-v1",
            "/ip4/37.27.11.202/udp/443/quic-v1",
            "/ip4/37.27.11.202/udp/500/quic-v1",
            "/ip4/37.27.11.202/udp/995/quic-v1",
         ]),
         ("12D3KooWEZXjE41uU4EL2gpkAQeDXYok6wghN7wwNVPF5bwkaNfS", &[
            "/dnsaddr/bootstrap.libp2p.io",
         ]),
         ("QmNnooDu7bfjPFoTZYxMNLWUQJyrVwtbZg5gBMjTezGAJN", &[
            "/dnsaddr/bootstrap.libp2p.io",
         ]),
         ("QmQCU2EcMqAqQPR2i9bChDtGNJchTbq5TbXJJ16u19uLTa", &[
            "/dnsaddr/bootstrap.libp2p.io",
         ]),
         ("QmZa1sAxajnQjVM8WjWXoMbmPd7NsWhfKsPkErzpm9wGkp", &[
            "/dnsaddr/bootstrap.libp2p.io",
         ]),
         ("QmbLHAnMoJPWSCR5Zhtx6BHJX9KiKNN6tpvbUcqanj75Nb", &[
            "/dnsaddr/bootstrap.libp2p.io",
         ]),
         ("QmcZf59bWwK5XFi76CZX8cbJ4BhTzzA3gU1ZjYZcYW3dwt", &[
            "/dnsaddr/bootstrap.libp2p.io",
         ]),
      ];

      let keypair = ed25519::Keypair::generate();

      Self {
         peer_id: PeerId(p2p::PeerId::from(p2p_id::PublicKey::from(keypair.public()))),
         keypair: FileOrInline::Inline(Keypair(keypair)),

         interface: None,
         listen:    [
            "/ip4/0.0.0.0/tcp/0",
            "/ip6/::/tcp/0",
            "/ip4/0.0.0.0/udp/0/quic-v1",
            "/ip6/::/udp/0/quic-v1",
         ]
         .iter()
         .map(|address| p2p::Multiaddr::from_str(address).expect("literals must be valid"))
         .collect(),

         aliases: vec!["self".parse().expect("literals must be valid")],

         zone: None,

         peers: PEERS
            .iter()
            .map(|&(peer_id, addresses)| {
               (peer_id.parse().expect("literals must be valid"), Peer {
                  addresses: addresses
                     .iter()
                     .map(|address| {
                        p2p::Multiaddr::from_str(address).expect("literals must be valid")
                     })
                     .collect(),
                  aliases:   Vec::new(),
                  allow:     Vec::new(),
               })
            })
            .collect(),
      }
   }
}
