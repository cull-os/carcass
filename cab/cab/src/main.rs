use std::{
   fmt::Write as _,
   path::{
      Path,
      PathBuf,
   },
   sync::Arc,
};

use cab::{
   island,
   report::{
      self,
      Contextful as _,
   },
   runtime,
   syntax,
};
use clap::Parser as _;
use ust::{
   COLORS,
   Display as _,
   Write,
   report::PositionStr,
   style::StyledExt,
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

   let out = &mut terminal::stdout();
   let err = &mut terminal::stderr();

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
         let expression = parse_oracle.parse(syntax::tokenize(&source)).extractlnln(
            err,
            &island::display!(leaf),
            &source,
         )?;

         let compile_oracle = runtime::compile_oracler();
         let code = compile_oracle.compile(expression.as_ref()).extractlnln(
            err,
            &island::display!(leaf),
            &source,
         )?;

         code.display_styled(out).context(FAIL_STDOUT)?;
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
                     let style = COLORS[kind as usize];

                     write(out, &slice.style(style))
                  } else {
                     writeln!(out, "{kind:?} {slice:?}")
                  }
                  .context(FAIL_STDOUT)?;
               }
            },

            Dump::Syntax => {
               let parse_oracle = syntax::parse_oracle();
               let expression = parse_oracle.parse(syntax::tokenize(&source)).extractlnln(
                  err,
                  &island::display!(leaf),
                  &source,
               )?;

               write!(out, "{node:#?}", node = expression.parent().unwrap())
                  .context(FAIL_STDOUT)?;
            },
         }
      },
   }

   out.finish()?;
   err.finish()?;

   report::Termination::success()
}
