use libp2p as p2p;

use crate::{
   Program,
   config,
   ip,
};

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case", tag = "command")]
pub enum Request {
   AddPeer {
      address: p2p::Multiaddr,

      #[serde(default)]
      allow: Vec<String>,
   },
   RemovePeer {
      peer_id: p2p::PeerId,
   },
}

#[derive(Debug, serde::Serialize)]
#[serde(untagged)]
pub enum Response {
   Ok { ok: String },
   Error { error: String },
}

impl<P: ip::Policy> Program<P> {
   pub fn handle_request(&mut self, request: Request) -> Response {
      match request {
         Request::AddPeer { address, allow } => {
            let ok = format!("added peer {address}");

            self.add_peer(&config::Peer { address, allow });

            Response::Ok { ok }
         },
         Request::RemovePeer { peer_id } => {
            let ok = format!("removed peer {peer_id}");

            self.remove_peer(peer_id);

            Response::Ok { ok }
         },
      }
   }
}
