use anyhow::Context;
use cab_ast::*;
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
    /// in the form of an unambigious S-expression.
    AstDump { expr: String },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::AstDump { expr } => {
            let expr: Expr = expr.parse().with_context(|| "failed to parse expression")?;

            println!("{}", expr.format_as_s_expr());
        },
    }

    Ok(())
}
