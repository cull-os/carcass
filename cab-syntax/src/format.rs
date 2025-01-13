//! Formatting utilities for [`Expression`]s.
use std::{
    fmt,
    io,
};

use yansi::Paint;

use crate::{
    node::{
        self,
        *,
    },
    token::Token,
    RowanNode,
    COLORS,
};

/// Formats the given node as an S-expression. The node must be a valid
/// [`Expression`] or this function will panic.
pub fn s(formatter: &mut impl io::Write, node: &RowanNode) -> io::Result<()> {
    Formatter::new(formatter).s(node)
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

    fn write_delimited(&mut self, delimiter: char, content: &str) -> io::Result<()> {
        write!(
            self.inner,
            "{delimiter}{content}{delimiter}",
            delimiter = delimiter.green().bold(),
            content = content.green()
        )
    }

    fn s_identifier_as_string(&mut self, identifier: &Identifier) -> io::Result<()> {
        match identifier.value() {
            IdentifierValue::Simple(token) => self.write_delimited('"', token.text()),

            IdentifierValue::Complex(complex) => {
                self.write('"'.green().bold())?;

                self.s_parted(
                    complex
                        .parts()
                        .filter(|part| !matches!(part, InterpolationPart::Delimiter(_))),
                )?;

                self.write('"'.green().bold())
            },
        }
    }

    fn s_parted<T: Token>(
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
                    self.s(&interpolation.expression().unwrap())?;
                    self.write(")".yellow())?;
                },
            }
        }

        Ok(())
    }

    fn s(&mut self, node: &RowanNode) -> io::Result<()> {
        node::r#match! { node =>
            Error as _error => {
                self.write("error".red().bold())
            },

            Parenthesis as parenthesis => {
                self.bracket_start("(")?;
                self.s(&parenthesis.expression())?;
                self.bracket_end(")")
            },

            List as list => {
                self.bracket_start("[")?;

                let items: Vec<_> = list.items().collect();

                for item in items.iter() {
                    self.write(" ")?;
                    self.s(item)?;
                }

                if !items.is_empty() {
                    self.write(" ")?;
                }
                self.bracket_end("]")
            },

            AttributeSet as set => {
                self.bracket_start("{")?;

                let inherits: Vec<_> = set.inherits().collect();
                let attributes: Vec<_> = set.attributes().collect();

                for (index, inherit) in inherits.iter().enumerate() {
                    self.write(" ")?;
                    self.s(&inherit.identifier())?;

                    if index + 1 != inherits.len() {
                        self.write(",")?;
                    }
                }

                if inherits.len() + attributes.len() != 0 {
                    self.write(",")?;
                }

                for (index, attribute) in attributes.iter().enumerate() {
                    self.write(" ")?;

                    let mut identifiers = attribute.path().identifiers();
                    if let Some(first) = identifiers.next() {
                        self.s(&first)?;
                    }
                    for identifier in identifiers {
                        self.write(".")?;
                        self.s(&identifier)?;
                    }

                    self.write(" = ")?;
                    self.s(&attribute.value())?;

                    if index + 1 != attributes.len() {
                        self.write(",")?;
                    }
                }

                if !inherits.is_empty() || !attributes.is_empty() {
                    self.write(" ")?;
                }
                self.bracket_end("}")
            },

            AttributeSelect as select => {
                self.bracket_start("(")?;

                self.write_delimited('`', ".")?;
                self.write(" ")?;
                self.s_identifier_as_string(&select.identifier())?;
                self.write(" ")?;
                self.s(&select.expression())?;

                self.bracket_end(")")
            },

            AttributeCheck as check => {
                self.bracket_start("(")?;

                self.write_delimited('`', "?")?;
                self.write(" ")?;
                self.s(&check.expression())?;

                for attribute in check.attributes() {
                    self.write(" ")?;
                    self.s(&attribute)?;
                }

                self.bracket_end(")")
            },

            Lambda as lambda => {
                self.bracket_start("(")?;

                match lambda.parameter() {
                    LambdaParameter::LambdaParameterIdentifier(parameter) => self.s(&parameter.identifier())?,
                    LambdaParameter::LambdaParameterPattern(parameter) => {
                        self.bracket_start("{")?;

                        let attributes: Vec<_> = parameter.attributes().collect();

                        for (index, attribute) in attributes.iter().enumerate() {
                            self.s(&attribute.identifier())?;

                            if let Some(default) = attribute.default() {
                                self.write(" ? ")?;
                                self.s(&default)?;
                            }

                            if index + 1 != attributes.len() {
                                self.write(", ")?;
                            }
                        }

                        if !attributes.is_empty() {
                            self.write(" ")?;
                        }
                        self.bracket_end("}")?;
                    },
                }

                self.write(": ")?;
                self.s(&lambda.expression().unwrap())?;
                self.bracket_end(")")
            },

            Application as application => {
                self.bracket_start("(")?;

                self.s(&application.left_expression())?;
                self.write(" ")?;
                self.s(&application.right_expression())?;

                self.bracket_end(")")
            },

            PrefixOperation as operation => {
                self.bracket_start("(")?;

                self.write_delimited('`', match operation.operator() {
                    PrefixOperator::Swwallation => "+",
                    PrefixOperator::Negation => "-",

                    PrefixOperator::Not => "not",
                })?;
                self.write(" ")?;
                self.s(&operation.expression())?;

                self.bracket_end(")")
            },

            InfixOperation as operation => {
                self.bracket_start("(")?;

                let operator = match operation.operator() {
                    InfixOperator::Apply => None,
                    InfixOperator::Pipe => {
                        self.s(&operation.right_expression().unwrap())?;
                        self.write(" ")?;
                        self.s(&operation.left_expression())?;

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
                    InfixOperator::Implication => Some("->"),

                    InfixOperator::Addition => Some("+"),
                    InfixOperator::Subtraction => Some("-"),
                    InfixOperator::Multiplication => Some("*"),
                    InfixOperator::Power => Some("**"),
                    InfixOperator::Division => Some("/"),

                    InfixOperator::And => Some("and"),
                    InfixOperator::Or => Some("or"),
                };

                if let Some(operator) = operator {
                    self.write_delimited('`', operator)?;
                    self.write(" ")?;
                }

                self.s(&operation.left_expression())?;
                self.write(" ")?;
                self.s(&operation.right_expression().unwrap())?;

                self.bracket_end(")")
            },

            Path as path => self.s_parted(path.parts()),

            Identifier as identifier => {
                match identifier.value() {
                    IdentifierValue::Simple(token) => self.write(match token.text() {
                        boolean @ ("true" | "false") => boolean.magenta().bold(),
                        inexistent @ ("null" | "undefined") => inexistent.cyan().bold(),
                        import @ "import" => import.yellow().bold(),
                        identifier => identifier.new(),
                    }),

                    IdentifierValue::Complex(complex) => self.s_parted(complex.parts()),
                }
            },

            SString as sstring => self.s_parted(sstring.parts()),

            Island as island => self.s_parted(island.parts()),

            Number as number => {
                match number.value() {
                    NumberValue::Integer(token) => self.write(token.value().blue().bold()),
                    NumberValue::Float(token) => self.write(token.value().blue().bold()),
                }
            },

            IfElse as if_else => {
                self.bracket_start("(")?;

                if if_else.false_expression().is_some() {
                    self.write_delimited('`', "if else")?;
                } else {
                    self.write_delimited('`', "if")?;
                }
                self.write(" ")?;

                self.s(&if_else.condition().unwrap())?;
                self.write(" ")?;
                self.s(&if_else.true_expression().unwrap())?;

                if let Some(false_expression) = if_else.false_expression() {
                    self.write(" ")?;
                    self.s(&false_expression)?;
                }

                self.bracket_end(")")
            },

            else => unreachable!(),
        }
    }
}
