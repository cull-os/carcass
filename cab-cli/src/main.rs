use std::{
    io,
    io::Write,
    process,
};

use cab::syntax;
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

#[derive(Subcommand, Debug, Clone)]
enum Command {
    /// Various commands related to debugging.
    Dump {
        #[command(subcommand)]
        command: Dump,

        // TODO: https://github.com/thepacketgeek/clap-stdin/issues/9
        /// The file to dump.
        #[clap(default_value = "-", global = true)]
        file: FileOrStdin,
    },
}

#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
enum Dump {
    /// Dump the provided file's tokens.
    Token {
        /// If specified, the output will be colored instead of typed.
        #[arg(long, short, global = true)]
        color: bool,
    },

    /// Dump the provided file's syntax.
    Syntax,

    /// Dump the provided file's abstract syntax tree
    /// in the form of an unambigious Cab expression
    /// that is very similar to Lisp.
    Ast,
}

impl Dump {
    fn run(self, file: FileOrStdin) {
        let contents = file.contents().unwrap_or_else(|error| {
            log::error!("failed to read file: {error}");
            process::exit(1);
        });

        let mut out = io::BufWriter::new(io::stdout());

        match self {
            Self::Token { color } => {
                for token in syntax::tokenize(&contents) {
                    let result = if color {
                        let on_color = syntax::COLORS[token.0 as usize];

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

            Self::Syntax | Self::Ast => {
                let parse = syntax::parse::<syntax::node::Expression>(&contents);

                if self == Self::Syntax {
                    write!(out, "{syntax}", syntax = parse.syntax())
                } else {
                    write!(out, "{tree}", tree = parse.tree())
                }
                .unwrap_or_else(|error| {
                    log::error!("failed to write to stdout: {error}");
                    process::exit(1);
                });
            },
        }
    }
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

    match cli.command {
        Command::Dump { file, command } => command.run(file),
    }

    Ok(())
}
