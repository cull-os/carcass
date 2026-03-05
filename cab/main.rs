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

   let source = path.read().await?.to_vec();
   let source = String::from_utf8(source).expect("source was created from UTF-8 string");
   let source = report::PositionStr::new(&source);

   // SOURCE -> TOKENS
   let tokens = syntax::tokenize(&source);

   match cli.dump_token {
      DumpToken::False => {},
      DumpToken::True => {
         for (kind, slice) in tokens.clone() {
            writeln!(out, "{kind:?} {slice:?}").expect("TODO move this inside the runtime");
         }
         writeln!(out).expect("TODO move inside the runtime");
      },
      DumpToken::Color => {
         for (kind, slice) in tokens.clone() {
            let style = COLORS[kind as usize];

            write(out, &slice.style(style)).expect("TODO move inside the runtime");
         }
         writeln!(out).expect("TODO move inside the runtime");
      },
   }

   // TOKENS -> PARSE
   let parse_oracle = syntax::ParseOracle::new();
   let parse = parse_oracle.parse(tokens);

   if cli.dump_syntax {
      // The Display of this already has a newline. So use write! instead.
      write!(out, "{node:#?}", node = &parse.node).expect("TODO move inside the runtime");
   }

   // EXTRACT EXPRESSION
   let expression = parse.extractlnln(err, &path, &source)?;

   // EXPRESSION -> LOWERED EXPRESSION
   let lower_oracle = syntax::LowerOracle::new();
   let lower = lower_oracle.lower(expression.as_ref());

   // TODO: Flag for displaying lower.

   // EXTRACT EXPRESSION
   let expression = lower.extractlnln(err, &path, &source)?;

   // EXPRESSION -> CODE
   let compile_oracle = runtime::CompileOracle::new();
   let code = compile_oracle.compile(expression).path(path.dupe());

   if cli.dump_code {
      code
         .display_styled(out)
         .expect("TODO move inside the runtime");
      writeln!(out).expect("TODO move inside the runtime");
   }

   // CODE -> THUNK
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
