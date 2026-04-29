#![feature(error_iter, try_blocks)]

use std::{
   error,
   io as blocking_io,
   mem,
   process as blocking_process,
   str::FromStr as _,
};

use radicle::storage::ReadStorage as _;
use tokio::{
   io::{
      self,
      AsyncBufReadExt as _,
   },
   process,
   sync::mpsc,
   task,
};
use tracing_subscriber::{
   filter as tracing_filter,
   util::{
      self as tracing_subscriber_util,
      SubscriberInitExt as _,
   },
};

mod broker;
mod config;
mod nix;

#[derive(Debug, thiserror::Error)]
enum Error {
   #[error("failed to parse tracing filter from environment variable")]
   ParseTracingFilter(#[from] tracing_filter::FromEnvError),

   #[error("failed to initialize tracing")]
   InitTracing(#[from] tracing_subscriber_util::TryInitError),

   #[error("failed to read request from stdin")]
   ReadRequest(#[source] broker::Error),

   #[error("request did not specify a commit")]
   NoCommit(#[source] broker::Error),

   #[error("failed to load configuration")]
   LoadConfig(#[from] config::Error),

   #[error("failed to write response to stdout")]
   WriteResponse(#[source] broker::Error),

   #[error("failed to spawn 'nix flake check'")]
   SpawnNix(#[source] io::Error),

   #[error("failed to read 'nix flake check' stderr")]
   ReadNixStderr(#[source] io::Error),

   #[error("failed to kill 'nix flake check'")]
   KillNix(#[source] io::Error),

   #[error("failed to wait for 'nix flake check'")]
   WaitNix(#[source] io::Error),

   #[error("failed to canonicalize repository path")]
   CanonicalizeRepoPath(#[source] io::Error),
}

#[bon::builder]
async fn check(
   join_set: &mut task::JoinSet<()>,
   config: &config::Config,
   flakeref: &str,
) -> Result<blocking_process::ExitStatus, Error> {
   let (sender, mut receiver) = mpsc::unbounded_channel::<Result<nix::Event, Vec<u8>>>();

   join_set.spawn(async move {
      while let Some(item) = receiver.recv().await {
         match item {
            Ok(event) => tracing::debug!(?event, "nix event"),
            Err(raw) => tracing::warn!(raw = ?raw, "raw nix stderr"),
         }
      }
   });

   tracing::info!(%flakeref, "Running 'nix flake check'");

   let mut child = process::Command::new("nix")
      .args(["flake", "check"])
      .arg("--no-write-lock-file")
      .args(config.verbosity().into_flags())
      .arg(flakeref)
      .stdout(blocking_process::Stdio::null())
      .stderr(blocking_process::Stdio::piped())
      .spawn()
      .map_err(Error::SpawnNix)?;

   let mut reader = io::BufReader::new(child.stderr.take().expect("stderr was piped"));

   macro_rules! abort {
      ($child:ident) => {{
         $child.start_kill().map_err(Error::KillNix)?;
         return $child.wait().await.map_err(Error::WaitNix);
      }};
   }

   let mut raw = Vec::new();
   let mut line = Vec::new();
   loop {
      if reader
         .read_until(b'\n', &mut line)
         .await
         .map_err(Error::ReadNixStderr)?
         == 0
      {
         break;
      }

      let line = mem::take(&mut line);

      let Ok(line) = str::from_utf8(&line) else {
         tracing::warn!(?line, "nix stderr contains invalid UTF-8");
         raw.extend_from_slice(&line);
         continue;
      };

      let Some(line) = line.strip_prefix(nix::PREFIX) else {
         tracing::warn!(?line, "nix stderr contains un-prefixed logs");
         raw.extend_from_slice(line.as_bytes());
         continue;
      };

      let event = match nix::Event::from_str(line) {
         Ok(event) => event,
         Err(error) => {
            tracing::error!(%error, "malformed nix event");
            raw.extend_from_slice(line.as_bytes());
            continue;
         },
      };

      if let raw = mem::take(&mut raw)
         && !raw.is_empty()
         && sender.send(Err(raw)).is_err()
      {
         abort!(child);
      }

      let Ok(()) = sender.send(Ok(event)) else {
         abort!(child);
      };
   }

   if let raw = mem::take(&mut raw)
      && !raw.is_empty()
      && sender.send(Err(raw)).is_err()
   {
      abort!(child);
   }

   child.wait().await.map_err(Error::WaitNix)
}

async fn real_main() -> Result<(), Error> {
   {
      const VARIABLE: &str = "RADICLE_CI_NIX_LOG";

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

   let request = broker::Request::from_reader(blocking_io::stdin()).map_err(Error::ReadRequest)?;
   let config = config::Config::load()?;

   {
      let run_id = broker::RunId::generate();
      broker::Response::Triggered {
         info_url: config.run_url([run_id.to_string()]),
         run_id,
      }
      .to_writer(blocking_io::stdout())
      .map_err(Error::WriteResponse)?;
   }

   let status = {
      let flakeref = format!(
         "git+{file_url}?rev={tip_oid}",
         file_url = url::Url::from_file_path(
            &config
               .profile()
               .storage
               .path_of(&request.repo_id())
               .canonicalize()
               .map_err(Error::CanonicalizeRepoPath)?
         )
         .expect("path was canonicalized"),
         tip_oid = request.tip_oid().map_err(Error::NoCommit)?,
      );

      let mut join_set = task::JoinSet::new();

      let status = check()
         .join_set(&mut join_set)
         .config(&config)
         .flakeref(&flakeref)
         .call()
         .await?;

      join_set.join_all().await;

      status
   };

   {
      broker::Response::Finished {
         result: if status.success() {
            broker::RunResult::Success
         } else {
            broker::RunResult::Failure
         },
      }
      .to_writer(blocking_io::stdout())
      .map_err(Error::WriteResponse)?;
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

      blocking_process::exit(1);
   }
}
