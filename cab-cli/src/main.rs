use std::{
    io::{
        self,
        Write,
    },
    process,
};

use cab_syntax::Tokenizer;
use clap::{
    Parser,
    Subcommand,
};
use clap_stdin::FileOrStdin;
use clap_verbosity_flag::{
    InfoLevel,
    Verbosity,
};
use colored::Colorize;
use log::Level;

#[derive(Parser)]
#[command(name = "cab", version, about)]
struct Cli {
    #[command(flatten)]
    verbosity: Verbosity<InfoLevel>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Dump the provided file.
    TokenDump {
        #[clap(default_value = "-")]
        file: FileOrStdin,
    },

    /// Dump the provided file's abstract syntax tree
    /// in the form of an unambigious Cab expression
    /// that is very similar to Lisp.
    AstDump {
        #[clap(default_value = "-")]
        file: FileOrStdin,
    },
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let cli = Cli::parse();

    // Trying to imitate clap to get a consistent experience.
    env_logger::Builder::new()
        .filter_level(cli.verbosity.log_level_filter())
        .format(|buffer, record| {
            let level = match record.level() {
                Level::Error => "error:".red().bold(),
                Level::Warn => "warn:".yellow().bold(),
                Level::Info => "info:".green().bold(),
                Level::Debug => "debug:".blue().bold(),
                Level::Trace => "trace:".cyan().bold(),
            };

            writeln!(buffer, "{level} {arguments}", arguments = record.args())
        })
        .init();

    let mut out = io::BufWriter::new(io::stdout());

    match cli.command {
        Command::TokenDump { file } => {
            let expression = file.contents().unwrap_or_else(|error| {
                log::error!("failed to read file: {error}");
                process::exit(1);
            });

            for token in Tokenizer::new(&expression) {
                writeln!(out, "{kind:?} {slice:?}", kind = token.0, slice = token.1)
                    .unwrap_or_else(|error| {
                        log::error!("failed to write to stdout: {error}");
                        process::exit(1);
                    });
            }
        },
        Command::AstDump { .. } => {
            log::error!("not implemented yet");
            process::exit(1);
        },
    }

    Ok(())
}
