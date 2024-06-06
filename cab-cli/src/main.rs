use anyhow::Context;
use cab_syntax::*;
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
    /// Dump the provided expression's abstract syntax tree
    /// in the form of an unambigious Cab expression that is
    /// very similar to Lisp.
    AstDump { expression: String },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::AstDump { expression } => {
            let expression: Expr = expression
                .parse()
                .with_context(|| "failed to parse expression")?;

            println!("{expression}");
        },
    }

    Ok(())
}
