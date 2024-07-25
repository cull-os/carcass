use std::{
    io::Write as _,
    process,
};

use cab::syntax::{
    self,
    ParseError,
};
use clap::Parser as _;
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
    Paint as _,
};

#[derive(clap::Parser)]
#[command(version, about)]
struct Cli {
    #[command(flatten)]
    verbosity: Verbosity<InfoLevel>,

    #[command(subcommand)]
    command: Command,
}

#[derive(clap::Subcommand, Debug, Clone)]
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

#[derive(clap::Subcommand, Debug, Clone, Copy)]
enum Dump {
    /// Dump the provided file's tokens.
    Token {
        /// If specified, the output will be colored instead of typed.
        #[arg(long, short, global = true)]
        color: bool,
    },

    /// Dump the provided file's syntax.
    Syntax,

    /// Dump the provided file as a colored S-expression.
    Clean,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    yansi::whenever(Condition::TTY_AND_COLOR);

    let (mut out, err) = {
        let choice = if yansi::is_enabled() {
            termcolor::ColorChoice::Always
        } else {
            termcolor::ColorChoice::Never
        };

        (
            termcolor::StandardStream::stdout(choice),
            termcolor::StandardStream::stderr(choice),
        )
    };

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
        Command::Dump { file, command } => {
            let name = file.filename().to_owned();
            let contents = file.contents().unwrap_or_else(|error| {
                log::error!("failed to read file: {error}");
                process::exit(1);
            });

            let mut files = SimpleFiles::new();
            let file_id = files.add(name, &contents);

            match command {
                Dump::Token { color } => {
                    for (kind, slice) in syntax::tokenize(&contents) {
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

                Dump::Syntax | Dump::Clean => {
                    let parse = syntax::parse::<syntax::node::Root>(&contents);

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

                    if matches!(command, Dump::Syntax) {
                        write!(out, "{syntax:#?}", syntax = parse.syntax())
                    } else {
                        syntax::format_s_expression(&mut out, &parse.node())
                    }
                    .unwrap_or_else(|error| {
                        log::error!("failed to write to stdout: {error}");
                        process::exit(1);
                    });
                },
            }
        },
    }
}
