#![feature(error_iter)]

use std::{
   error,
   fmt::Write as _,
   io as blocking_io,
   path,
   process,
};

use clap::Parser as _;
use route67::{
   config,
   socket,
};
use tokio::{
   fs,
   io,
};
use tracing_subscriber::{
   filter as tracing_filter,
   util::{
      self as tracing_subscriber_util,
      SubscriberInitExt as _,
   },
};
use ust::{
   Write as _,
   report,
   style::StyledExt as _,
   terminal,
};

#[derive(Debug, thiserror::Error)]
enum Error {
   #[error("failed to parse tracing filter from environment variable")]
   ParseTracingFilter(#[from] tracing_filter::FromEnvError),

   #[error("failed to initialize tracing")]
   InitTracing(#[from] tracing_subscriber_util::TryInitError),

   #[error("could not determine config directory")]
   CouldNotDetermineConfigDir,

   #[cfg(target_os = "linux")]
   #[error("could not determine runtime directory (XDG_RUNTIME_DIR unset)")]
   XdgRuntimeDirUnset,

   #[error("invalid config")]
   InvalidConfig(#[from] Box<config::Error>),

   #[error("failed to create config directory '{path}'", path = .path.display())]
   CreateConfigDir {
      path:   path::PathBuf,
      #[source]
      source: io::Error,
   },

   #[error("failed to write config to '{path}'", path = .path.display())]
   WriteConfig {
      path:   path::PathBuf,
      #[source]
      source: io::Error,
   },

   #[error("failed to read config from '{path}'", path = .path.display())]
   ReadConfig {
      path:   path::PathBuf,
      #[source]
      source: io::Error,
   },

   #[error("failed to bind control socket")]
   BindControlSocket(#[from] socket::ConnectError),

   #[error("daemon connection closed")]
   DaemonConnectionClosed,

   #[error("daemon did not respond")]
   DaemonDidNotRespond,

   #[error("daemon returned error: '{error}'")]
   Daemon { error: String },

   #[error("unexpected response from daemon: '{response:?}'")]
   UnexpectedResponse { response: socket::Response },

   #[error(transparent)]
   Route67(#[from] Box<route67::Error>),
}

fn config_path() -> Result<path::PathBuf, Error> {
   Ok(dirs::config_dir()
      .ok_or(Error::CouldNotDetermineConfigDir)?
      .join("route67")
      .join("config.toml"))
}

#[cfg_attr(
   not(target_os = "linux"),
   expect(clippy::unnecessary_wraps, reason = "fallible on Linux")
)]
fn socket_path() -> Result<path::PathBuf, Error> {
   #[cfg(target_os = "linux")]
   {
      Ok(dirs::runtime_dir()
         .ok_or(Error::XdgRuntimeDirUnset)?
         .join("route67")
         .join("socket"))
   }

   #[cfg(target_os = "macos")]
   {
      Ok(path::PathBuf::from("/var/run/route67.socket"))
   }

   #[cfg(target_os = "windows")]
   {
      Ok(path::PathBuf::from(r"\\.\pipe\route67"))
   }
}

#[derive(clap::Parser)]
#[command(version, about)]
struct Cli {
   #[command(subcommand)]
   command: Command,
}

#[derive(clap::Subcommand, Debug, Clone)]
enum Command {
   /// Start the node.
   Start {
      /// Generate config if it doesn't exist.
      #[arg(long)]
      generate_config: bool,
   },

   /// Query peer status.
   Status { peer_id: libp2p::PeerId },

   /// Map a peer.
   Map {
      peer_id: libp2p::PeerId,

      #[arg(long = "address")]
      addresses: Vec<libp2p::Multiaddr>,

      #[arg(long)]
      allow: Vec<String>,
   },

   /// Unmap a peer.
   Unmap { peer_id: libp2p::PeerId },
}

async fn send_request(request: socket::Request) -> Result<socket::Response, Error> {
   let mut exchanges =
      socket::connect::<{ socket::Type::Client }, socket::Response, socket::Request>(
         &socket_path()?
      )
      .await?;

   let Some((response_receiver, request_sender)) = exchanges.recv().await else {
      return Err(Error::DaemonConnectionClosed);
   };

   let Ok(()) = request_sender.send(request) else {
      return Err(Error::DaemonConnectionClosed);
   };

   response_receiver
      .await
      .map_err(|_| Error::DaemonDidNotRespond)
}

#[expect(clippy::match_wildcard_for_single_variants)]
async fn real_main() -> Result<(), Error> {
   {
      const VARIABLE: &str = "ROUTE67_LOG";

      tracing_subscriber::fmt()
         .with_writer(blocking_io::stderr)
         .with_env_filter(
            tracing_subscriber::EnvFilter::builder()
               .with_env_var(VARIABLE)
               .with_default_directive(tracing_filter::Directive::from(
                  tracing_filter::LevelFilter::INFO,
               ))
               .from_env()?,
         )
         .finish()
         .try_init()?;
   }

   let cli = Cli::parse();

   let err = &mut terminal::stderr();

   match cli.command {
      Command::Start { generate_config } => {
         let path = config_path()?;

         let config = match fs::read_to_string(&path).await {
            Ok(content) => {
               tracing::info!(path = %path.display(), "Using existing config");

               match route67::Config::try_from(content.as_str()) {
                  Ok(config) => config,
                  Err(config::Error::Parse(ref error)) => {
                     let report = if let Some(span) = error.span() {
                        report::Report::error("invalid config")
                           .primary(span, error.message().to_owned())
                     } else {
                        report::Report::error(error.message().to_owned())
                     };

                     let _ = err.write_report(
                        &report,
                        &path.display().to_string().yellow(),
                        &report::PositionStr::new(&content),
                     );

                     let _ = write!(err, "\n\n");

                     return Err(Error::InvalidConfig(Box::new(config::Error::Parse(
                        error.clone(),
                     ))));
                  },
                  Err(error) => return Err(Error::InvalidConfig(Box::new(error))),
               }
            },
            Err(error) if error.kind() == io::ErrorKind::NotFound && generate_config => {
               tracing::info!(path = %path.display(), "Generating config");

               let config = route67::Config::generate();

               if let Some(parent) = path.parent() {
                  fs::create_dir_all(parent).await.map_err(|source| {
                     Error::CreateConfigDir {
                        path: parent.to_owned(),
                        source,
                     }
                  })?;
               }

               let serialized = toml::to_string_pretty(&config)
                  .expect("generated config serialization must not fail");

               fs::write(&path, &serialized).await.map_err(|source| {
                  Error::WriteConfig {
                     path: path.clone(),
                     source,
                  }
               })?;

               config
            },
            Err(source) => {
               return Err(Error::ReadConfig {
                  path: path.clone(),
                  source,
               });
            },
         };

         Box::pin(
            route67::run()
               .config(config)
               .requests(
                  socket::connect::<{ socket::Type::Server }, socket::Request, socket::Response>(
                     &socket_path()?,
                  )
                  .await?,
               )
               .call(),
         )
         .await
         .map_err(|error| Error::Route67(Box::new(error)))?;
      },
      Command::Status { peer_id } => {
         let response = send_request(socket::Request::PeerStatus { peer_id }).await?;

         match response {
            socket::Response::PeerStatus { connections, .. } => {
               // TODO: Colors and stuff.
               for connection in connections {
                  println!("{connection}");
               }
            },
            socket::Response::Error { error } => return Err(Error::Daemon { error }),
            response => return Err(Error::UnexpectedResponse { response }),
         }
      },
      Command::Map {
         peer_id,
         addresses,
         allow,
      } => {
         let response = send_request(socket::Request::MapPeer {
            peer_id,
            addresses,
            allow,
         })
         .await?;

         match response {
            socket::Response::Ok { ok } => println!("{ok}"),
            socket::Response::Error { error } => return Err(Error::Daemon { error }),
            response => return Err(Error::UnexpectedResponse { response }),
         }
      },
      Command::Unmap { peer_id } => {
         let response = send_request(socket::Request::UnmapPeer { peer_id }).await?;

         match response {
            socket::Response::Ok { ok } => println!("{ok}"),
            socket::Response::Error { error } => return Err(Error::Daemon { error }),
            response => return Err(Error::UnexpectedResponse { response }),
         }
      },
   }

   Ok(())
}

#[tokio::main]
async fn main() {
   if let Err(error) = real_main().await {
      tracing::error!(%error, "Fatal error");

      for error in (&error as &dyn error::Error).sources().skip(1) {
         tracing::error!(%error, "Caused by");
      }

      process::exit(1);
   }
}
