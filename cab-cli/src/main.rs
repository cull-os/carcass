use std::{
    io::{
        self,
        Write,
    },
    process,
};

use cab_syntax::{
    Tokenizer,
    SYNTAX_COLORS,
};
use clap::{
    Parser,
    Subcommand,
};
use clap_stdin::FileOrStdin;
use clap_verbosity_flag::{
    InfoLevel,
    Verbosity,
};
use colored::{
    Colorize,
    CustomColor,
};
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
    /// Various commands related to debugging.
    Dump {
        /// If specified, the output will be colored instead of typed.
        #[arg(long, short, global = true)]
        color: bool,

        #[command(subcommand)]
        command: DumpCommand,
    },
}

#[derive(Subcommand)]
enum DumpCommand {
    /// Dump the provided file's tokens.
    Token {
        /// The file to dump the tokens of.
        #[clap(default_value = "-")]
        file: FileOrStdin,
    },

    /// Dump the provided file's abstract syntax tree
    /// in the form of an unambigious Cab expression
    /// that is very similar to Lisp.
    Ast {
        /// The file to dump the AST of.
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
        Command::Dump {
            color,
            command: DumpCommand::Token { file },
        } => {
            let expression = file.contents().unwrap_or_else(|error| {
                log::error!("failed to read file: {error}");
                process::exit(1);
            });

            for token in Tokenizer::new(&expression) {
                let result = if color {
                    let on_color = SYNTAX_COLORS[token.0 as usize];

                    let color = if (0.2126 * on_color.r as f32
                        + 0.7152 * on_color.g as f32
                        + 0.0722 * on_color.b as f32)
                        < 140.0
                    {
                        CustomColor::new(0xFF, 0xFF, 0xFF)
                    } else {
                        CustomColor::new(0, 0, 0)
                    };

                    write!(
                        out,
                        "{slice}",
                        slice = token.1.on_custom_color(on_color).custom_color(color)
                    )
                } else {
                    writeln!(out, "{kind:?} {slice:?}", kind = token.0, slice = token.1)
                };

                result.unwrap_or_else(|error| {
                    log::error!("failed to write to stdout: {error}");
                    process::exit(1);
                });
            }
        },

        _ => {
            log::error!("not implemented yet");
            process::exit(1);
        },
    }

    Ok(())
}
