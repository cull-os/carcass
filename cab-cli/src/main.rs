use std::{
    io,
    io::Write,
    process,
};

use cab::syntax::{
    self,
    ParseError,
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
use codespan_reporting::{
    diagnostic::{
        Diagnostic,
        Label,
    },
    files::SimpleFiles,
    term::{
        self,
        termcolor,
    },
};
use log::Level;
use yansi::{
    Condition,
    Paint,
};

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

        /// The file to dump.
        #[clap(default_value = "-", global = true)]
        file: FileOrStdin,
    },
}

#[derive(Subcommand, Debug, Clone)]
enum Dump {
    /// Dump the provided file's tokens.
    Token {
        /// If specified, the output will be colored instead of typed.
        #[arg(long, short, global = true)]
        color: bool,
    },

    /// Dump the provided file's syntax.
    Syntax,

    /// Dump the provided file in a clean manner.
    Clean,
}

impl Dump {
    fn run(self, file: FileOrStdin) {
        let contents = file.contents().unwrap_or_else(|error| {
            log::error!("failed to read file: {error}");
            process::exit(1);
        });

        // TODO: https://github.com/thepacketgeek/clap-stdin/issues/2#issuecomment-2225371594
        let mut files = SimpleFiles::new();
        let file_id = files.add("todofixthis.cab", &contents);

        let mut out = io::BufWriter::new(io::stdout());

        let err = termcolor::StandardStream::stderr(if yansi::is_enabled() {
            termcolor::ColorChoice::Always
        } else {
            termcolor::ColorChoice::Never
        });

        match self {
            Self::Token { color } => {
                for syntax::TokenizerToken(kind, slice) in syntax::tokenize(&contents) {
                    let result = if color {
                        let style = syntax::COLORS[kind as usize];

                        write!(out, "{slice}", slice = slice.paint(style))
                    } else {
                        writeln!(out, "{kind:?} {slice:?}")
                    };

                    result.unwrap_or_else(|error| {
                        log::error!("failed to write to stdout: {error}");
                        process::exit(1);
                    });
                }
            },

            Self::Syntax | Self::Clean => {
                let parse = syntax::parse(&contents);

                let error_config = term::Config::default();

                for error in parse.errors() {
                    let diagnostic = Diagnostic::error()
                        .with_message("syntax error")
                        .with_labels(vec![Label::primary(
                            file_id,
                            match error {
                                ParseError::Unexpected { at, .. } => {
                                    at.start().into()..at.end().into()
                                },

                                ParseError::RecursionLimitExceeded { at } => {
                                    let as_usize = Into::<u32>::into(*at) as usize;

                                    as_usize..as_usize
                                },
                            },
                        )
                        .with_message(format!("{error}"))]);

                    term::emit(&mut err.lock(), &error_config, &files, &diagnostic).ok();
                }

                if matches!(self, Self::Syntax) {
                    write!(out, "{syntax:#?}", syntax = parse.syntax())
                } else {
                    write!(out, "{root}", root = parse.root())
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

    yansi::whenever(Condition::TTY_AND_COLOR);

    // Trying to imitate clap to get a consistent experience.
    env_logger::Builder::new()
        .filter_level(cli.verbosity.log_level_filter())
        .format(|buffer, record| {
            writeln!(
                buffer,
                "{level} {arguments}",
                level = match record.level() {
                    Level::Error => "error:".red().bold(),
                    Level::Warn => "warn:".yellow().bold(),
                    Level::Info => "info:".green().bold(),
                    Level::Debug => "debug:".blue().bold(),
                    Level::Trace => "trace:".cyan().bold(),
                },
                arguments = record.args()
            )
        })
        .init();

    match cli.command {
        Command::Dump { file, command } => command.run(file),
    }

    Ok(())
}
