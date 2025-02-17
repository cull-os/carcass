use std::{
    io::{
        self,
        Write as _,
    },
    path::{
        Path,
        PathBuf,
    },
    sync::Arc,
};

use cab::{
    error::{
        self,
        Contextful as _,
    },
    island,
    report,
    syntax,
};
use cab_island::Entry;
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

        /// The file to dump. If set to '-', stdin is read.
        #[clap(default_value = "-", global = true)]
        path: PathBuf,
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
async fn main() -> error::Termination {
    let cli = Cli::parse();

    report::init(cli.verbosity.log_level_filter());

    let (mut out, mut err) = (io::stdout(), io::stderr());

    match cli.command {
        Command::Dump { path, command } => {
            let leaf: Arc<dyn island::Leaf> = if path == Path::new("-") {
                Arc::new(island::stdin())
            } else {
                Arc::new(island::fs(path))
            };

            let source = leaf.clone().read().await?.to_vec();

            let entry: Arc<dyn Entry> = leaf.clone();

            let source = String::from_utf8(source).with_context(|| {
                format!(
                    "failed to convert '{leaf}' to an UTF-8 string",
                    leaf = entry.to_display()
                )
            })?;

            match command {
                Dump::Token { color } => {
                    for (kind, slice) in syntax::tokenize(&source) {
                        if color {
                            let style = syntax::COLORS[kind as usize];

                            write!(out, "{slice}", slice = slice.paint(style))
                        } else {
                            writeln!(out, "{kind:?} {slice:?}")
                        }
                        .context("failed to write to stdout")?;
                    }
                },

                Dump::Syntax | Dump::Parenthesize => {
                    let oracle = syntax::oracle();
                    let parse = oracle.parse(syntax::tokenize(&source));

                    for report in parse.reports {
                        writeln!(err, "{report}", report = report.with(leaf.clone()).await).ok();
                    }

                    if let Dump::Syntax = command {
                        write!(out, "{node:#?}", node = parse.node)
                    } else {
                        syntax::format::parenthesize(&mut out, parse.expression.as_ref())
                    }
                    .context("failed to write to stdout")?;
                },
            }
        },
    }

    error::Termination::success()
}
