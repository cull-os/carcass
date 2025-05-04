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
   format::{
      self,
      style::StyleExt as _,
   },
   island,
   report::{
      self,
      Contextful as _,
      PositionStr,
   },
   runtime,
   syntax,
};
use clap::Parser as _;

const FAIL_STDERR: &str = "failed to write to stderr";
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
      expression: String,
   },

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
}

#[tokio::main]
async fn main() -> report::Termination {
   let cli = Cli::parse();

   format::init();

   let (mut out, mut err) = (io::stdout(), io::stderr());

   match cli.command {
      Command::Compile { expression: source } => {
         let leaf: Arc<dyn island::Leaf> = if source == "-" {
            Arc::new(island::stdin())
         } else {
            Arc::new(island::blob(source))
         };

         let source = leaf.clone().read().await?.to_vec();

         let source = String::from_utf8(source).with_context(|| {
            format!(
               "failed to convert {leaf} to an UTF-8 string",
               leaf = island::display!(leaf)
            )
         })?;

         let source = PositionStr::new(&source);

         let parse_oracle = syntax::parse_oracle();
         let expression = parse_oracle
            .parse(syntax::tokenize(&source))
            .result()
            .map_err(|error| error.locate(island::display!(leaf), &source))?;

         let compile_oracle = runtime::compile_oracler();
         let code = compile_oracle
            .compile(expression.as_ref())
            .result()
            .map_err(|error| error.locate(island::display!(leaf), &source))?;

         writeln!(out, "{code}").context(FAIL_STDOUT)?;
      },

      Command::Dump { path, command } => {
         let leaf: Arc<dyn island::Leaf> = if path == Path::new("-") {
            Arc::new(island::stdin())
         } else {
            Arc::new(island::fs(path))
         };

         let source = leaf.clone().read().await?.to_vec();

         let source = String::from_utf8(source).with_context(|| {
            format!(
               "failed to convert {leaf} to an UTF-8 string",
               leaf = island::display!(leaf)
            )
         })?;

         let source = PositionStr::new(&source);

         match command {
            Dump::Token { color } => {
               for (kind, slice) in syntax::tokenize(&source) {
                  if color {
                     let style = format::COLORS[kind as usize];

                     write!(out, "{slice}", slice = slice.style(style))
                  } else {
                     writeln!(out, "{kind:?} {slice:?}")
                  }
                  .context(FAIL_STDOUT)?;
               }
            },

            Dump::Syntax => {
               let parse_oracle = syntax::parse_oracle();
               let parse = parse_oracle.parse(syntax::tokenize(&source));

               for report in parse.reports {
                  writeln!(
                     err,
                     "{report}\n",
                     report = report.locate(island::display!(leaf), &source)
                  )
                  .context(FAIL_STDERR)?;
               }

               write!(out, "{node:#?}", node = parse.node).context(FAIL_STDOUT)?;
            },
         }
      },
   }

   report::Termination::success()
}
