use std::{
   env,
   fmt::Write as _,
   io as std_io,
};

use clap::Parser as _;
use cyn::ResultExt as _;
use tokio::io::{
   self,
   AsyncReadExt as _,
};
use tracing_subscriber::filter as tracing_filter;
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

   /// Node related commands.
   Node {
      #[command(subcommand)]
      command: Node,
   },

   /// Show inbox.
   Inbox,

   /// List peers.
   Peers,

   /// Ping peer.
   Ping,
}

#[derive(clap::Subcommand, Debug, Clone)]
enum Config {
   /// Generate a new configuration.
   Generate,
}

#[derive(clap::Subcommand, Debug, Clone)]
enum Node {
   /// Start node. Configuration is read from stdin.
   Start,

   /// Reload already running node. Configuration is read from stdin.
   Reload,
}

#[tokio::main]
async fn main() -> cyn::Termination {
   {
      const VARIABLE: &str = "CON_LOG";

      tracing_subscriber::fmt()
         .with_writer(std_io::stderr)
         .with_env_filter(
            tracing_subscriber::EnvFilter::builder()
               .with_env_var(VARIABLE)
               .with_default_directive(tracing_filter::LevelFilter::INFO.into())
               .from_env()
               .chain_err_with(|| {
                  format!(
                     "failed to initialize tracing from environment variable '{VARIABLE}' with \
                      content '{content}'",
                     content = env::var_os(VARIABLE)
                        .expect("unset variable cannot be invalid")
                        .display(),
                  )
               })?,
         )
         .try_init()
         .chain_err("failed to initialize tracing")?;
   }

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

      Command::Node {
         command: Node::Start,
      } => {
         let mut config = String::new();

         io::stdin()
            .read_to_string(&mut config)
            .await
            .chain_err("failed to read config from stdin")?;

         let config: con::Config = match toml::from_str(&config) {
            Ok(config) => config,
            Err(error) => {
               let report = if let Some(span) = error.span() {
                  report::Report::error("invalid config").primary(span, error.message().to_owned())
               } else {
                  report::Report::error(error.message().to_owned())
               };

               err.write_report(
                  &report,
                  &"<stdin>".yellow(),
                  &report::PositionStr::new(&config),
               )
               .chain_err(FAIL_STDERR)?;

               write!(err, "\n\n").chain_err(FAIL_STDERR)?;

               cyn::bail!("failed to parse config due to 1 previous error");
            },
         };

         con::run(config).await?;
      },
      Command::Node {
         command: Node::Reload,
      } => todo!(),

      Command::Inbox => todo!(),
      Command::Peers => todo!(),
      Command::Ping => todo!(),
   }

   cyn::Termination::success()
}
