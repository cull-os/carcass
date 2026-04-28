#![feature(error_iter, try_blocks)]

use std::{
   error,
   io,
   process,
};

use radicle::storage::ReadStorage as _;
use tracing_subscriber::{
   filter as tracing_filter,
   util::{
      self as tracing_subscriber_util,
      SubscriberInitExt as _,
   },
};

mod config;
mod message;
mod nix;

#[derive(Debug, thiserror::Error)]
enum Error {
   #[error("failed to parse tracing filter from environment variable")]
   ParseTracingFilter(#[from] tracing_filter::FromEnvError),

   #[error("failed to initialize tracing")]
   InitTracing(#[from] tracing_subscriber_util::TryInitError),

   #[error("failed to read request from stdin")]
   ReadRequest(#[source] message::Error),

   #[error("request did not specify a commit")]
   NoCommit(#[source] message::Error),

   #[error("failed to load configuration")]
   LoadConfig(#[from] config::Error),

   #[error("failed to write response to stdout")]
   WriteResponse(#[source] message::Error),

   #[error("failed to spawn 'nix flake check'")]
   SpawnNix(#[source] io::Error),

   #[error("failed to canonicalize repository path")]
   CanonicalizeRepoPath(#[source] io::Error),
}

fn real_main() -> Result<(), Error> {
   {
      const VARIABLE: &str = "RADICLE_CI_NIX_LOG";

      tracing_subscriber::fmt()
         .with_writer(io::stderr)
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

   let request = message::Request::from_reader(io::stdin()).map_err(Error::ReadRequest)?;

   let config = config::Config::load()?;

   let repo_path = config
      .profile()
      .storage
      .path_of(&request.repo_id())
      .canonicalize()
      .map_err(Error::CanonicalizeRepoPath)?;

   let run_id = message::RunId::generate();

   message::Response::Triggered {
      info_url: config.run_url([run_id.to_string()]),
      run_id,
   }
   .to_writer(io::stdout())
   .map_err(Error::WriteResponse)?;

   let status = {
      let flakeref = format!(
         "git+file://{path}?rev={tip_oid}",
         path = repo_path.display(),
         tip_oid = request.tip_oid().map_err(Error::NoCommit)?,
      );

      tracing::info!(%flakeref, "Running 'nix flake check'");

      process::Command::new("nix")
         .args(["flake", "check", "--no-write-lock-file", &flakeref])
         .status()
         .map_err(Error::SpawnNix)?
   };

   message::Response::Finished {
      result: if status.success() {
         message::RunResult::Success
      } else {
         tracing::warn!(?status, "'nix flake check' failed");
         message::RunResult::Failure
      },
   }
   .to_writer(io::stdout())
   .map_err(Error::WriteResponse)?;

   Ok(())
}

fn main() {
   if let Err(error) = real_main() {
      tracing::error!(%error, "Fatal error");

      for error in (&error as &dyn error::Error).sources().skip(1) {
         tracing::error!(%error, "Caused by");
      }

      process::exit(1);
   }
}
