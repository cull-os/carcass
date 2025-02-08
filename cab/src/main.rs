use std::{
    io,
    io::Write as _,
    process,
};

use cab::{
    report,
    syntax,
};
use clap::Parser as _;
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

    report::init(cli.verbosity.log_level_filter());

    let (mut out, mut err) = (io::stdout(), io::stderr());

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

                    // let mut files = diagnostic_files::SimpleFiles::new();
                    // let input_file = files.add(name, &contents);

                    // for error in &parse.errors {
                    //     let diagnostic = diagnostic::Diagnostic::error()
                    //         .with_message("syntax error")
                    //         .with_labels(vec![
                    //             diagnostic::Label::primary(input_file, error.range)
                    //                 .with_message(&*error.reason),
                    //         ]);

                    //     term::emit(&mut err.lock(), &Default::default(), &files,
                    // &diagnostic).ok(); }

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
