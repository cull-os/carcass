use std::{
    io::Write as _,
    process,
};

use cab::syntax;
use clap::Parser as _;
use codespan_reporting::{
    diagnostic,
    files as diagnostic_files,
    term,
};
use yansi::Paint as _;

#[derive(clap::Parser)]
#[command(version, about)]
struct Cli {
    #[command(flatten)]
    verbosity: clap_verbosity_flag::Verbosity<clap_verbosity_flag::InfoLevel>,

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
        file: clap_stdin::FileOrStdin,
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

    /// Dump the provided file with parentheses to disambiguate.
    Parenthesize,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    yansi::whenever(yansi::Condition::TTY_AND_COLOR);

    let (mut out, err) = {
        let choice = if yansi::is_enabled() {
            term::termcolor::ColorChoice::Always
        } else {
            term::termcolor::ColorChoice::Never
        };

        (
            term::termcolor::StandardStream::stdout(choice),
            term::termcolor::StandardStream::stderr(choice),
        )
    };

    // Trying to imitate clap to get a consistent experience.
    env_logger::Builder::new()
        .filter_level(cli.verbosity.log_level_filter())
        .format(|buffer, record| {
            let level = match record.level() {
                log::Level::Error => "error:".red().bold(),
                log::Level::Warn => "warn:".yellow().bold(),
                log::Level::Info => "info:".green().bold(),
                log::Level::Debug => "debug:".blue().bold(),
                log::Level::Trace => "trace:".cyan().bold(),
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

                Dump::Syntax | Dump::Parenthesize => {
                    let parse = syntax::parse::<_, syntax::node::Expression>(
                        syntax::tokenize(&contents),
                        Default::default(),
                    );

                    let mut files = diagnostic_files::SimpleFiles::new();
                    let input_file = files.add(name, &contents);

                    let diagnostic = diagnostic::Diagnostic::error()
                        .with_message("syntax error")
                        .with_labels(
                            parse
                                .errors
                                .into_iter()
                                .map(|error| {
                                    diagnostic::Label::primary(input_file, error.at)
                                        .with_message(error.reason)
                                })
                                .collect(),
                        );

                    term::emit(
                        &mut err.lock(),
                        &term::Config::default(),
                        &files,
                        &diagnostic,
                    )
                    .ok();

                    if let Dump::Syntax = command {
                        write!(out, "{syntax:#?}", syntax = parse.syntax)
                    } else {
                        syntax::format::parenthesize(&mut out, &parse.syntax.first_child().unwrap())
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
