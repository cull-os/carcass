use cab_syntax::Tokenizer;
use clap::{
    Parser,
    Subcommand,
};
use clap_stdin::FileOrStdin;

#[derive(Parser)]
#[command(name = "cab", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Dump the provided file.
    TokenDump {
        #[clap(default_value = "-")]
        file: FileOrStdin,
    },

    /// Dump the provided file's abstract syntax tree
    /// in the form of an unambigious Cab expression
    /// that is very similar to Lisp.
    AstDump {
        #[clap(default_value = "-")]
        file: FileOrStdin,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::TokenDump { file } => {
            Tokenizer::new(&file.contents()?)
                .into_iter()
                .for_each(|token| println!("{kind:?} {slice:?}", kind = token.0, slice = token.1));
        },
        Command::AstDump { .. } => {
            todo!();
        },
    }

    Ok(())
}
