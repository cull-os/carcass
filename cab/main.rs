use std::fmt::Write as _;

use cab::{
   runtime,
   syntax,
   util::suffix::Arc as _,
};
use clap::Parser as _;
use cyn::ResultExt as _;
use dup::Dupe as _;
use ranged::Span;
use rpds::ListSync as List;
use runtime::{
   Value,
   value,
};
use ust::{
   COLORS,
   Display as _,
   report,
   style::StyledExt as _,
   terminal,
   write,
};

#[derive(clap::Parser)]
#[command(version, about)]
struct Cli {
   /// Print the result of every `Language.tokenize` call.
   #[arg(long, default_value = "false")]
   dump_token: DumpToken,

   /// Print the result of every `Language.parse` call.
   #[arg(long, default_value = "false")]
   dump_syntax: bool,

   /// Print the result of every `Language.compile` call.
   #[arg(long, default_value = "false")]
   dump_code: bool,

   /// The expression to `evaluate`.
   expression: Vec<String>,
}

#[derive(clap::ValueEnum, Debug, Clone, Copy)]
enum DumpToken {
   False,
   True,
   Color,
}

#[tokio::main]
async fn main() -> cyn::Termination {
   let cli = Cli::parse();

   let out = &mut terminal::stdout();
   let err = &mut terminal::stderr();

   let expression = match &*cli.expression {
      &[] => unimplemented!("repl"),
      parts => parts.join(" "),
   };

   let path = value::Path::new()
      .root(value::path::blob(Value::from(value::SString::from(&*expression))).arc())
      .subpath(List::new_sync());

   // TODO: position_cache in Path.
   let source = path.read().await?.to_vec();
   let source = String::from_utf8(source).expect("source was created from UTF-8 string");
   let source = report::PositionStr::new(&source);

   let parse_oracle = syntax::ParseOracle::new();
   let parse = parse_oracle.parse(syntax::tokenize(&source).inspect(|&(kind, slice)| {
      match cli.dump_token {
         DumpToken::False => {},
         DumpToken::True => {
            writeln!(out, "{kind:?} {slice:?}").expect("TODO move this inside the runtime");
         },
         DumpToken::Color => {
            let style = COLORS[kind as usize];

            write(out, &slice.style(style)).expect("TODO move inside the runtime");
         },
      }
   }));

   if let DumpToken::True | DumpToken::Color = cli.dump_token {
      writeln!(out).expect("TODO move inside the runtime");
   }

   if cli.dump_syntax {
      // The Display of this already has a newline. So use write! instead.
      write!(out, "{node:#?}", node = &parse.node).expect("TODO move inside the runtime");
   }

   let expression = parse.extractlnln(err, &path, &source)?;

   let compile_oracle = runtime::CompileOracle::new();
   let code = compile_oracle
      .compile(expression.as_ref())
      .path(path.dupe())
      .extractlnln(err, &path, &source)?;

   if cli.dump_code {
      code
         .display_styled(out)
         .expect("TODO move inside the runtime");
      writeln!(out).expect("TODO move inside the runtime");
   }

   let thunk = value::Thunk::forceable(code.arc())
      .scopes(
         runtime::Scopes::new().push(runtime::Scope::from(&value::attributes::new! {
            "foo": Value::from(value::string::new!("AAAA")),
            "bar": Value::from(value::attributes::new! {
               "baz": Value::Boolean(false),
            }),
            "true": Value::Boolean(true),
            "false": Value::Boolean(false),
            // "fee": Value::from(value::Thunk::forceable_native(|| {
            //    eprintln!("[BACKGROUND PROCESS] Selling personal data to mastercard...");
            //    Value::from(value::string::new!("Your transaction has been successful."))
            // })),
         })),
      )
      .location(value::Location::new(path, Span::at(0_u32, source.len())));

   thunk
      .force(&runtime::State {
         parse_oracle,
         compile_oracle,
      })
      .await;

   let (_, value) = thunk.get().await;

   value
      .display_styled(out)
      .chain_err("failed to display value")?;

   cyn::Termination::success()
}

#[cfg(test)]
mod tests {
   use clap::CommandFactory as _;

   use super::*;

   #[test]
   fn cli() {
      Cli::command().debug_assert();
   }
}
