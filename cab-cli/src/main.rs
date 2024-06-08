use std::{
    fs,
    path::Path,
};

use cab_syntax::Tokenizer;
use clap::{
    Parser,
    Subcommand,
};

#[derive(Parser)]
#[command(name = "cab", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Dump the provided expressions tokens.
    TokenDump { expression_or_file: String },
    /// Dump the provided expression's abstract syntax tree
    /// in the form of an unambigious Cab expression that is
    /// very similar to Lisp.
    AstDump { expression_or_file: String },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::TokenDump { expression_or_file } => {
            let expression = if Path::new(&expression_or_file).is_file() {
                fs::read_to_string(expression_or_file)?
            } else {
                expression_or_file
            };

            Tokenizer::new(&expression)
                .into_iter()
                .for_each(|token| println!("{kind:?} {slice:?}", kind = token.0, slice = token.1));
        },
        Command::AstDump { .. } => {
            todo!();
        },
    }

    Ok(())
}
