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
   island,
   syntax,
   why::{
      self,
      Contextful as _,
   },
};
use cab_why::{
   PositionStr,
   ReportSeverity,
   bail,
};
use clap::Parser as _;
use yansi::Paint as _;

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
async fn main() -> why::Termination {
   let cli = Cli::parse();

   yansi::whenever(yansi::Condition::TTY_AND_COLOR);

   let (mut out, mut err) = (io::stdout(), io::stderr());

   match cli.command {
      // Pretty bad but will clean up later.
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

         let syntax_oracle = syntax::oracle();
         let parse = syntax_oracle.parse(syntax::tokenize(&source));

         let mut fail = 0;
         for report in parse.reports {
            if report.severity >= ReportSeverity::Error {
               fail += 1;
            }

            writeln!(
               err,
               "{report}",
               report = report.with(island::display!(leaf), &source),
            )
            .ok();
         }

         if fail > 0 {
            bail!(
               "compilation failed due to {fail} previous error{s}",
               s = if fail == 1 { "" } else { "s" }
            );
         }

         let expression = parse.expression;

         let compile_oracle = cab::runtime::compile_oracler();
         let compile = compile_oracle.compile(expression.as_ref());

         let mut fail = 0;
         for report in compile.reports {
            if report.severity >= ReportSeverity::Error {
               fail += 1;
            }

            writeln!(
               err,
               "{report}",
               report = report.with(island::display!(leaf), &source),
            )
            .ok();
         }

         if fail > 0 {
            bail!(
               "compilation failed due to {fail} previous error{s}",
               s = if fail == 1 { "" } else { "s" }
            );
         }

         let code = compile.code;

         let _ = code;
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
                     let style = syntax::COLORS[kind as usize];

                     write!(out, "{slice}", slice = slice.paint(style))
                  } else {
                     writeln!(out, "{kind:?} {slice:?}")
                  }
                  .context("failed to write to stdout")?;
               }
            },

            Dump::Syntax => {
               let oracle = syntax::oracle();
               let parse = oracle.parse(syntax::tokenize(&source));

               for report in parse.reports {
                  writeln!(
                     err,
                     "{report}",
                     report = report.with(island::display!(leaf), &source)
                  )
                  .ok();
               }

               write!(out, "{node:#?}", node = parse.node).context("failed to write to stdout")?;
            },
         }
      },
   }

   why::Termination::success()
}
