use std::{
   fmt::Write as _,
   path::PathBuf,
};

use clap::Parser as _;
use cyn::ResultExt as _;
use tokio::fs;
use ust::{
   Write,
   report,
   style::StyledExt as _,
   terminal,
};

const FAIL_STDOUT: &str = "failed to write to stdout";
const FAIL_STDERR: &str = "failed to write to stderr";

#[derive(clap::Parser)]
#[command(version, about)]
struct Cli {
   #[command(subcommand)]
   command: Command,
}

#[derive(clap::Subcommand, Debug, Clone)]
enum Command {
   /// Start daemon.
   Start {
      /// Path to daemon configuration.
      #[arg(long)]
      config: PathBuf,
   },

   /// Show inbox.
   Inbox,

   /// List peers.
   Peers,

   /// Ping peer.
   Ping,

   /// Start network.
   Up,

   /// Stop network.
   Down,
}

#[tokio::main]
async fn main() -> cyn::Termination {
   let cli = Cli::parse();

   let out = &mut terminal::stdout();
   let err = &mut terminal::stderr();

   match cli.command {
      Command::Start {
         config: config_path,
      } => {
         let config = fs::read_to_string(&config_path).await.chain_err_with(|| {
            format!(
               "failed to read config from '{path}'",
               path = config_path.display(),
            )
         })?;

         let config = report::PositionStr::new(&config);

         let config = match toml::from_str::<con::Config>(&config) {
            Ok(config) => config,
            Err(error) => {
               let mut report = report::Report::error(error.message().to_owned());

               if let Some(span) = error.span() {
                  report.push_primary(span, "here");
               }

               err.write_report(&report, &config_path.display().yellow(), &config)
                  .chain_err(FAIL_STDERR)?;

               write!(err, "\n\n").chain_err(FAIL_STDERR)?;

               cyn::bail!("failed to parse config");
            },
         };
      },
      Command::Inbox => todo!(),
      Command::Peers => todo!(),
      Command::Ping => todo!(),
      Command::Up => todo!(),
      Command::Down => todo!(),
   }

   out.finish()?;
   err.finish()?;

   cyn::Termination::success()
}
