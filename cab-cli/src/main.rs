#![feature(proc_macro_byte_character, proc_macro_c_str_literals)]

use clap::{
    Parser,
    Subcommand,
};

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Evaluate a Cab expresion
    Eval,
    /// Enter an interactive Cab REPL
    Repl,
    /// Format Cab code
    Fmt,
}

#[tokio::main]
async fn main() {}
