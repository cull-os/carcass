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
    TokenDump { expression: String },
    /// Dump the provided expression's abstract syntax tree
    /// in the form of an unambigious Cab expression that is
    /// very similar to Lisp.
    AstDump { expression: String },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::TokenDump { expression } => {
            Tokenizer::new(&expression)
                .into_iter()
                .for_each(|token| println!("{kind:?} {slice:?}", kind = token.0, slice = token.1));
        },
        Command::AstDump {
            expression: _expression,
        } => {
            todo!();
        },
    }

    Ok(())
}
