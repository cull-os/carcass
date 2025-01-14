//! Formatting utilities for [`Expression`]s.
use std::{
    fmt,
    io,
};

use yansi::Paint;

use crate::{
    COLORS,
    RowanNode,
    node::{
        self,
        *,
    },
    token::Token,
};

/// Formats the given node with parentheses to disambiguate.
/// The node must be a valid [`Expression`] or this will panic.
pub fn parenthesize(formatter: &mut impl io::Write, node: &RowanNode) -> io::Result<()> {
    Formatter::new(formatter).parenthesize(node)
}

#[derive(Debug)]
struct Formatter<'a, W: io::Write> {
    inner: &'a mut W,

    bracket_count: u32,
}

impl<'a, W: io::Write> Formatter<'a, W> {
    fn new(inner: &'a mut W) -> Self {
        Self {
            inner,

            bracket_count: 0,
        }
    }

    fn paint_bracket<'b>(&self, bracket: &'b str) -> yansi::Painted<&'b str> {
        let style = COLORS[self.bracket_count as usize % COLORS.len()];
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

    fn parenthesize_parted<T: Token>(
        &mut self,
        parts: impl Iterator<Item = InterpolationPart<T>>,
    ) -> io::Result<()> {
        for part in parts {
            match part {
                InterpolationPart::Delimiter(token) => {
                    self.write(token.text().green().bold())?;
                },

                InterpolationPart::Content(token) => {
                    self.write(token.text().green())?;
                },

                InterpolationPart::Interpolation(interpolation) => {
                    self.write(r"\(".yellow())?;
                    self.parenthesize(&interpolation.expression().unwrap())?;
                    self.write(")".yellow())?;
                },
            }
        }

        Ok(())
    }

    fn parenthesize(&mut self, node: &RowanNode) -> io::Result<()> {
        node::r#match! { node =>
            Error as _error => {
                self.write("error".red().bold())
            },

            Parenthesis as parenthesis => {
                self.bracket_start("(")?;
                self.parenthesize(&parenthesis.expression())?;
                self.bracket_end(")")
            },

            List as list => {
                self.bracket_start("[")?;

                let mut items = list.items().peekable();
                while let Some(item) = items.next() {
                    self.parenthesize(&item)?;

                    if items.peek().is_some() {
                        self.write(", ")?;
                    }
                }

                self.bracket_end("]")
            },

            AttributeList as list => {
                self.bracket_start("{")?;

                // TODO: Pretty print.
                if let Some(expression) = list.expression() {
                    self.parenthesize(&expression)?;
                }

                self.bracket_end("}")
            },

            PrefixOperation as operation => {
                self.bracket_start("(")?;

                self.write(match operation.operator() {
                    PrefixOperator::Swwallation => "+",
                    PrefixOperator::Negation => "-",

                    PrefixOperator::Not => "!",
                })?;
                self.write(" ")?;
                self.parenthesize(&operation.expression())?;

                self.bracket_end(")")
            },

            InfixOperation as operation => {
                self.bracket_start("(")?;

                let operator = match operation.operator() {
                    InfixOperator::Select => Some("."),
                    InfixOperator::Check => Some("?"),

                    InfixOperator::Sequence => Some(";"),
                    InfixOperator::Same => Some(","),

                    InfixOperator::ImplicitApply | InfixOperator::Apply => None,
                    InfixOperator::Pipe => {
                        self.parenthesize(&operation.right_expression().unwrap())?;
                        self.write(" ")?;
                        self.parenthesize(&operation.left_expression())?;

                        return self.bracket_end(")");
                    },

                    InfixOperator::Concat => Some("++"),

                    InfixOperator::Update => Some("//"),

                    InfixOperator::Equal => Some("=="),
                    InfixOperator::NotEqual => Some("!="),
                    InfixOperator::LessOrEqual => Some("<="),
                    InfixOperator::Less => Some("<"),
                    InfixOperator::MoreOrEqual => Some(">="),
                    InfixOperator::More => Some(">"),

                    InfixOperator::Addition => Some("+"),
                    InfixOperator::Subtraction => Some("-"),
                    InfixOperator::Multiplication => Some("*"),
                    InfixOperator::Power => Some("^"),
                    InfixOperator::Division => Some("/"),

                    InfixOperator::And => Some("&&"),
                    InfixOperator::Or => Some("||"),
                    InfixOperator::Implication => Some("->"),

                    InfixOperator::Lambda => Some("=>"),
                    InfixOperator::Bind => Some(":="),
                };

                self.parenthesize(&operation.left_expression())?;
                self.write(" ")?;

                if let Some(operator) = operator {
                    self.write(operator)?;
                    self.write(" ")?;
                }

                self.parenthesize(&operation.right_expression().unwrap())?;

                self.bracket_end(")")
            },

            SuffixOperation as operation => {
                self.bracket_start("(")?;

                self.parenthesize(&operation.expression())?;
                self.write(" ")?;
                self.write(match operation.operator() {
                    SuffixOperator::Same => ",",
                    SuffixOperator::Sequence => ";",
                })?;

                self.bracket_end(")")
            },

            Path as path => self.parenthesize_parted(path.parts()),

            Identifier as identifier => {
                match identifier.value() {
                    IdentifierValue::Simple(token) => self.write(match token.text() {
                        boolean @ ("true" | "false") => boolean.magenta().bold(),
                        inexistent @ ("null" | "undefined") => inexistent.cyan().bold(),
                        import @ "import" => import.yellow().bold(),
                        identifier => identifier.new(),
                    }),

                    IdentifierValue::Complex(complex) => self.parenthesize_parted(complex.parts()),
                }
            },

            SString as sstring => self.parenthesize_parted(sstring.parts()),

            Island as island => self.parenthesize_parted(island.parts()),

            Number as number => {
                match number.value() {
                    NumberValue::Integer(token) => self.write(token.value().blue().bold()),
                    NumberValue::Float(token) => self.write(token.value().blue().bold()),
                }
            },

            IfIs as if_is => {
                self.bracket_start("(")?;

                self.write("if ".red().bold())?;
                self.parenthesize(&if_is.expression().unwrap())?;
                self.write(" is ".red().bold())?;
                self.parenthesize(&if_is.match_expression().unwrap())?;

                self.bracket_end(")")
            },

            IfElse as if_else => {
                self.bracket_start("(")?;

                self.write("if ".red().bold())?;
                self.parenthesize(&if_else.condition().unwrap())?;
                self.write(" then ".red().bold())?;
                self.parenthesize(&if_else.true_expression().unwrap())?;

                if let Some(false_expression) = if_else.false_expression() {
                    self.write(" else".red().bold())?;
                    self.parenthesize(&false_expression)?;
                }

                self.bracket_end(")")
            },

            else => unreachable!(),
        }
    }
}
