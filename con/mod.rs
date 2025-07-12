use std::path::PathBuf;

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
   interface: String,

   private_key_path: PathBuf,

   listen_addresses: Vec<p2p::Multiaddr>,

   peers: Vec<Peer>,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Peer {
   public_key: String,

   name: Option<String>,
}
