//! Formatting utilities for [`node::Expression`]s.
use std::{
   fmt,
   io,
};

use yansi::Paint as _;

use crate::{
   COLORS,
   node,
   red,
};

/// Formats the given node with parentheses to disambiguate.
pub fn parenthesize(
   writer: &mut impl io::Write,
   expression: node::ExpressionRef<'_>,
) -> io::Result<()> {
   Formatter::new(writer).parenthesize(expression)
}

#[derive(Debug)]
struct Formatter<'write, W: io::Write> {
   inner: &'write mut W,

   bracket_count: usize,
}

impl<'write, W: io::Write> Formatter<'write, W> {
   fn new(inner: &'write mut W) -> Self {
      Self {
         inner,

         bracket_count: 0,
      }
   }

   fn paint_bracket<'b>(&self, bracket: &'b str) -> yansi::Painted<&'b str> {
      let style = COLORS[self.bracket_count % COLORS.len()];
      bracket.paint(style)
   }

   fn bracket_start(&mut self, bracket: &str) -> io::Result<()> {
      write!(
         self.inner,
         "{painted}",
         painted = self.paint_bracket(bracket)
      )?;
      self.bracket_count += 1;

      Ok(())
   }

   fn bracket_end(&mut self, bracket: &str) -> io::Result<()> {
      self.bracket_count -= 1;
      write!(
         self.inner,
         "{painted}",
         painted = self.paint_bracket(bracket)
      )
   }

   fn write(&mut self, painted: impl fmt::Display) -> io::Result<()> {
      write!(self.inner, "{painted}")
   }

   fn write_parted(&mut self, parted: &impl node::Parted) -> io::Result<()> {
      for part in parted.parts() {
         match part {
            node::InterpolatedPartRef::Delimiter(token) => {
               self.write(token.text().green().bold())?;
            },

            node::InterpolatedPartRef::Content(token) => {
               self.write(token.text().green())?;
            },

            node::InterpolatedPartRef::Interpolation(interpolation) => {
               self.write(r"\(".yellow())?;
               self.parenthesize(interpolation.expression())?;
               self.write(")".yellow())?;
            },
         }
      }

      Ok(())
   }

   fn parenthesize(&mut self, expression: node::ExpressionRef<'_>) -> io::Result<()> {
      match expression {
         node::ExpressionRef::Error(_error) => self.write("error".red().bold()),

         node::ExpressionRef::Parenthesis(parenthesis) => {
            if let Some(expression) = parenthesis.expression() {
               self.parenthesize(expression)?;
            }

            Ok(())
         },

         node::ExpressionRef::List(list) => {
            self.bracket_start("[")?;

            let mut items = list.items().peekable();
            if items.peek().is_some() {
               self.write(" ")?;
            }

            while let Some(item) = items.next() {
               self.parenthesize(item)?;

               if items.peek().is_some() {
                  self.write(",")?;
               }

               self.write(" ")?;
            }

            self.bracket_end("]")
         },

         node::ExpressionRef::Attributes(attributes) => {
            self.bracket_start("{")?;

            if let Some(expression) = attributes.expression() {
               self.write(" ")?;
               self.parenthesize(expression)?;
               self.write(" ")?;
            }

            self.bracket_end("}")
         },

         node::ExpressionRef::PrefixOperation(operation) => {
            self.bracket_start("(")?;

            self.write(operation.operator_token().text())?;
            self.write(" ")?;
            self.parenthesize(operation.right())?;

            self.bracket_end(")")
         },

         node::ExpressionRef::InfixOperation(operation) => {
            self.bracket_start("(")?;

            let operator = match operation.operator() {
               node::InfixOperator::ImplicitApply | node::InfixOperator::Apply => None,
               node::InfixOperator::Pipe => {
                  self.parenthesize(operation.right())?;
                  self.write(" ")?;
                  self.parenthesize(operation.left())?;

                  return self.bracket_end(")");
               },

               _ => operation.operator_token().map(|token| token.text()),
            };

            self.parenthesize(operation.left())?;
            self.write(" ")?;

            if let Some(operator) = operator {
               self.write(operator)?;
               self.write(" ")?;
            }

            self.parenthesize(operation.right())?;

            self.bracket_end(")")
         },

         node::ExpressionRef::SuffixOperation(operation) => {
            self.bracket_start("(")?;

            self.parenthesize(operation.left())?;
            self.write(" ")?;
            self.write(operation.operator_token().text())?;

            self.bracket_end(")")
         },

         node::ExpressionRef::Path(path) => self.write_parted(path),

         node::ExpressionRef::Bind(bind) => {
            self.write(bind.token_at().text())?;
            self.parenthesize(bind.identifier())
         },

         node::ExpressionRef::Identifier(identifier) => {
            match identifier.value() {
               node::IdentifierValueRef::Plain(token) => {
                  self.write(match token.text() {
                     boolean @ ("true" | "false") => boolean.magenta().bold(),
                     inexistent @ ("null" | "undefined") => inexistent.cyan().bold(),
                     import @ "import" => import.yellow().bold(),
                     identifier => identifier.new(),
                  })
               },

               node::IdentifierValueRef::Quoted(quoted) => self.write_parted(quoted),
            }
         },

         node::ExpressionRef::SString(string) => self.write_parted(string),

         node::ExpressionRef::Rune(rune) => self.write_parted(rune),

         node::ExpressionRef::Island(island) => {
            self.write_parted(island.header())?;

            for element in island.children_with_tokens().skip(1) {
               match element {
                  red::ElementRef::Node(node) => {
                     self.parenthesize(node.try_into().unwrap())?;
                  },
                  red::ElementRef::Token(token) => {
                     self.write(token.text().green())?;
                  },
               }
            }

            Ok(())
         },

         node::ExpressionRef::Integer(integer) => self.write(integer.value().blue().bold()),
         node::ExpressionRef::Float(float) => self.write(float.value().blue().bold()),

         node::ExpressionRef::If(if_) => {
            self.bracket_start("(")?;

            self.write(if_.token_if().text().red().bold())?;
            self.write(" ")?;
            self.parenthesize(if_.condition())?;
            self.write(" ")?;
            self.write(
               if_.token_then()
                  .map_or("then", |token| token.text())
                  .red()
                  .bold(),
            )?;
            self.parenthesize(if_.consequence())?;
            self.write(" else ".red().bold())?;
            self.parenthesize(if_.alternative())?;

            self.bracket_end(")")
         },
      }
   }
}
