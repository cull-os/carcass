use std::{
   fs,
   marker,
   path,
};

use libp2p as p2p;
use libp2p::futures::{
   SinkExt as _,
   StreamExt as _,
};
use serde::de as serde_de;
use tokio::{
   io,
   net,
   sync::{
      mpsc,
      oneshot,
   },
};
use tokio_util::codec::{
   self as codec,
   Framed,
   LinesCodec,
};

use crate::{
   Program,
   config,
   ip,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, marker::ConstParamTy)]
pub enum Type {
   Server,
   Client,
}

pub type Exchange<In, Out> = (oneshot::Receiver<In>, oneshot::Sender<Out>);

#[expect(clippy::cognitive_complexity)]
async fn handle_stream<
   const TYPE: Type,
   In: serde_de::DeserializeOwned + Send + 'static,
   Out: serde::Serialize + Send + Sync + 'static,
>(
   stream: net::UnixStream,
   sender: mpsc::UnboundedSender<Exchange<In, Out>>,
) {
   async fn read_message<T: serde_de::DeserializeOwned>(
      stream: &mut Framed<net::UnixStream, LinesCodec>,
   ) -> Option<T> {
      let line = match stream.next().await? {
         Ok(line) => line,
         Err(error) => {
            tracing::error!(%error, "Failed to read from socket");
            return None;
         },
      };

      match serde_json::from_str(&line) {
         Ok(value) => Some(value),
         Err(error) => {
            tracing::error!(%error, "Failed to deserialize socket message");
            None
         },
      }
   }

   async fn write_message<T: serde::Serialize>(
      stream: &mut Framed<net::UnixStream, LinesCodec>,
      value: &T,
   ) -> Result<(), codec::LinesCodecError> {
      let line = serde_json::to_string(value).expect("serialization must not fail");
      stream.send(line).await
   }

   let mut stream = Framed::new(stream, LinesCodec::new());

   loop {
      let (in_sender, in_receiver) = oneshot::channel::<In>();
      let (out_sender, out_receiver) = oneshot::channel::<Out>();

      match TYPE {
         Type::Client => {
            if let Err(error) = sender.send((in_receiver, out_sender)) {
               tracing::error!(?error, "Exchange receiver dropped");
               break;
            }

            let out_value = match out_receiver.await {
               Ok(out_value) => out_value,
               Err(error) => {
                  tracing::error!(%error, "Out sender dropped without sending");
                  break;
               },
            };
            if let Err(error) = write_message(&mut stream, &out_value).await {
               tracing::error!(%error, "Failed to write to socket");
               break;
            }

            let Some(in_value) = read_message(&mut stream).await else {
               break;
            };
            let _ = in_sender.send(in_value);
         },
         Type::Server => {
            let Some(in_value) = read_message(&mut stream).await else {
               break;
            };
            let _ = in_sender.send(in_value);

            if let Err(error) = sender.send((in_receiver, out_sender)) {
               tracing::error!(?error, "Exchange receiver dropped");
               break;
            }

            let out_value = match out_receiver.await {
               Ok(out_value) => out_value,
               Err(error) => {
                  tracing::error!(%error, "Out sender dropped without sending");
                  break;
               },
            };
            if let Err(error) = write_message(&mut stream, &out_value).await {
               tracing::error!(%error, "Failed to write to socket");
               break;
            }
         },
      }
   }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
   #[error("failed to connect to socket '{path}'", path = .path.display())]
   Connect {
      path:   path::PathBuf,
      #[source]
      source: io::Error,
   },

   #[error("failed to bind to socket '{path}'", path = .path.display())]
   Bind {
      path:   path::PathBuf,
      #[source]
      source: io::Error,
   },
}

pub async fn connect<
   const TYPE: Type,
   In: serde_de::DeserializeOwned + Send + 'static,
   Out: serde::Serialize + Send + Sync + 'static,
>(
   path: &path::Path,
) -> Result<mpsc::UnboundedReceiver<Exchange<In, Out>>, Error> {
   let (sender, receiver) = mpsc::unbounded_channel();

   match TYPE {
      Type::Client => {
         let stream = net::UnixStream::connect(path)
            .await
            .map_err(|source| Error::Connect { path: path.to_owned(), source })?;

         tracing::info!(address = ?stream.local_addr().expect("unix socket with path must have local address"), "Control socket listened");

         tokio::spawn(handle_stream::<TYPE, In, Out>(stream, sender));
      },
      Type::Server => {
         let _ = fs::remove_file(path);
         let listener =
            net::UnixListener::bind(path).map_err(|source| Error::Bind { path: path.to_owned(), source })?;

         tracing::info!(address = ?listener.local_addr().expect("unix socket with path must have local address"), "Control socket bound");

         tokio::spawn(async move {
            loop {
               let (stream, _) = match listener.accept().await {
                  Ok(connection) => connection,
                  Err(error) => {
                     tracing::error!(%error, "Control socket accept failed");
                     continue;
                  },
               };

               tokio::spawn(handle_stream::<TYPE, In, Out>(stream, sender.clone()));
            }
         });
      },
   }

   Ok(receiver)
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case", tag = "command")]
pub enum Request {
   TrustPeer {
      address: p2p::Multiaddr,

      #[serde(default)]
      allow: Vec<String>,
   },
   DistrustPeer {
      peer_id: p2p::PeerId,
   },
   PeerStatus {
      peer_id: p2p::PeerId,
   },
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum Response {
   PeerStatus {
      ok:                     String,
      // TODO: Order by activity (index 0 = last active)
      connections:            Vec<p2p::Multiaddr>,
      connection_last_active: Option<p2p::Multiaddr>,
   },
   Ok {
      ok: String,
   },
   Error {
      error: String,
   },
}

impl<P: ip::Policy> Program<P> {
   pub fn handle_request(&mut self, request: Request) -> Response {
      match request {
         Request::TrustPeer { address, allow } => {
            match self.trust_peer(&config::Peer {
               address: address.clone(),
               allow,
            }) {
               Ok(()) => {
                  Response::Ok {
                     ok: format!("trusted peer '{address}'"),
                  }
               },
               Err(error) => Response::Error { error },
            }
         },
         Request::DistrustPeer { peer_id } => {
            let ok = format!("distrusted peer '{peer_id}'");

            self.distrust_peer(peer_id);

            Response::Ok { ok }
         },
         Request::PeerStatus { peer_id } => {
            let peer_connections = self.connections.get(&peer_id);

            Response::PeerStatus {
               ok: format!("peer '{peer_id}' status"),

               connections: peer_connections
                  .iter()
                  .flat_map(|connections| {
                     connections.iter().map(|&(_, ref address)| address.clone())
                  })
                  .collect(),

               connection_last_active: self
                  .swarm
                  .behaviour()
                  .ip
                  .last_active_connection_id(&peer_id)
                  .and_then(|connection_id| {
                     peer_connections?.iter().find_map(|&(id, ref address)| {
                        (id == connection_id).then(|| address.clone())
                     })
                  }),
            }
         },
      }
   }
}
