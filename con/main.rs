use std::fmt::Write as _;

use clap::Parser as _;
use cyn::ResultExt as _;
use tokio::io::{
   self,
   AsyncReadExt as _,
};
use ust::{
   Write as _,
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
   /// Configuration related commands,
   Config {
      #[command(subcommand)]
      command: Config,
   },

   /// Start daemon. Configuration is read from stdin.
   Start,

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

#[derive(clap::Subcommand, Debug, Clone)]
enum Config {
   /// Generate a new configuration.
   Generate,
}

#[tokio::main]
async fn main() -> cyn::Termination {
   let cli = Cli::parse();

   let out = &mut terminal::stdout();
   let err = &mut terminal::stderr();

   match cli.command {
      Command::Config {
         command: Config::Generate,
      } => {
         let config = con::Config::generate()?;

         let config = toml::to_string_pretty(&config)
            .chain_err("failed to generate config, this is a bug")?;

         writeln!(out, "{config}").chain_err(FAIL_STDOUT)?;
      },

      Command::Start => {
         let mut config = String::new();

         io::stdin()
            .read_to_string(&mut config)
            .await
            .chain_err("failed to read config from stdin")?;

         let config: con::Config = match toml::from_str(&config) {
            Ok(config) => config,
            Err(error) => {
               let mut report = report::Report::error(error.message().to_owned());

               if let Some(span) = error.span() {
                  report.push_primary(span, "here");
               }

               err.write_report(
                  &report,
                  &"<stdin>".yellow(),
                  &report::PositionStr::new(&config),
               )
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
