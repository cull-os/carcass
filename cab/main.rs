use std::{
   fmt::Write as _,
   sync::Arc,
};

use cab::{
   error::{
      self,
      Contextful as _,
   },
   runtime,
   syntax,
};
use clap::Parser as _;
use rpds::ListSync as List;
use runtime::value;
use ust::{
   COLORS,
   Display as _,
   Write as _,
   report,
   style::StyledExt as _,
   terminal,
   write,
};

const FAIL_STDOUT: &str = "failed to write to stdout";

#[derive(clap::Parser)]
#[command(version, about)]
struct Cli {
   #[command(subcommand)]
   command: Command,
}

#[derive(clap::Subcommand, Debug, Clone)]
enum Command {
   // Compile an expression.
   Compile {
      #[clap(default_value = "-")]
      source: String,
   },

   /// Various commands related to debugging.
   Dump {
      #[command(subcommand)]
      command: Dump,

      /// The expression to dump. If set to '-', stdin is read.
      #[clap(default_value = "-", global = true)]
      source: String,
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
}

#[tokio::main]
async fn main() -> error::Termination {
   let cli = Cli::parse();

   let out = &mut terminal::stdout();
   let err = &mut terminal::stderr();

   let (Command::Compile { ref source } | Command::Dump { ref source, .. }) = cli.command;

   let path: Arc<dyn value::path::Root> = if source == "-" {
      Arc::new(value::path::standard())
   } else {
      Arc::new(value::path::blob(runtime::Value::String(
         source.as_str().into(),
      )))
   };
   let path = value::Path::new(path, List::new_sync());

   let source = path.read().await?.to_vec();
   let source = String::from_utf8(source).expect("source was created from UTF-8 string");
   let source = report::PositionStr::new(&source);

   match cli.command {
      Command::Compile { .. } => {
         let parse_oracle = syntax::ParseOracle::new();
         let expression = parse_oracle
            .parse(syntax::tokenize(&source))
            .extractlnln(err, &path, &source)?;

         let compile_oracle = runtime::CompileOracle::new();
         let code = compile_oracle
            .compile(path.clone(), expression.as_ref())
            .extractlnln(err, &path, &source)?;

         code.display_styled(out).context(FAIL_STDOUT)?;
      },

      Command::Dump { command, .. } => {
         match command {
            Dump::Token { color } => {
               for (kind, slice) in syntax::tokenize(&source) {
                  if color {
                     let style = COLORS[kind as usize];

                     write(out, &slice.style(style))
                  } else {
                     writeln!(out, "{kind:?} {slice:?}")
                  }
                  .context(FAIL_STDOUT)?;
               }
            },

            Dump::Syntax => {
               let parse_oracle = syntax::ParseOracle::new();
               let expression = parse_oracle
                  .parse(syntax::tokenize(&source))
                  .extractlnln(err, &path, &source)?;

               write!(out, "{node:#?}", node = expression.parent().unwrap())
                  .context(FAIL_STDOUT)?;
            },
         }
      },
   }

   out.finish()?;
   err.finish()?;

   error::Termination::success()
}
