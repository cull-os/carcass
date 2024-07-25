//! Formatting utilities for [`Expression`]s.
use std::io;

use yansi::Paint as _;

use crate::{
    match_node,
    node::*,
    token::Token,
    RowanNode,
    COLORS,
};

/// Formats the given node as an S-expression. The node must be a valid
/// [`Expression`] or this function will panic.
pub fn s(formatter: &mut impl io::Write, node: &RowanNode) -> io::Result<()> {
    s_impl(formatter, node, &mut 0)
}

fn s_impl_parted<T: Token>(
    parts: impl Iterator<Item = InterpolationPart<T>>,
    formatter: &mut impl io::Write,
    bracket_count: &mut u32,
) -> io::Result<()> {
    for part in parts {
        match part {
            InterpolationPart::Delimiter(token) => {
                write!(
                    formatter,
                    "{delimiter}",
                    delimiter = token.text().green().bold()
                )?;
            },

            InterpolationPart::Content(token) => {
                write!(formatter, "{content}", content = token.text().green())?;
            },

            InterpolationPart::Interpolation(interpolation) => {
                write!(formatter, "{start}", start = "${".yellow())?;
                s_impl(formatter, &interpolation.expression(), bracket_count)?;
                write!(formatter, "{end}", end = "}".yellow())?;
            },
        }
    }

    Ok(())
}

fn s_impl(
    formatter: &mut impl io::Write,
    node: &RowanNode,
    bracket_count: &mut u32,
) -> io::Result<()> {
    fn get_bracket_style(bracket_count: u32) -> yansi::Style {
        COLORS[bracket_count as usize % COLORS.len()]
    }

    match_node! { node =>
        Root as root => {
            s_impl(formatter, &root.expression(), bracket_count)
        },

        Error as _error => {
            write!(formatter, "{error}", error = "error".red().bold())
        },

        Parenthesis as parenthesis => {
            write!(formatter, "{left}", left = "(".paint(get_bracket_style(*bracket_count)))?;
            *bracket_count += 1;
            s_impl(formatter, &parenthesis.expression(), bracket_count)?;
            *bracket_count -= 1;
            write!(formatter, "{right}", right = ")".paint(get_bracket_style(*bracket_count)))
        },

        List as list => {
            write!(formatter, "{left}", left = "[".paint(get_bracket_style(*bracket_count)))?;
            *bracket_count += 1;

            let items: Vec<_> = list.items().collect();

            for item in items.iter() {
                write!(formatter, " ")?;
                s_impl(formatter, item, bracket_count)?;
            }

            *bracket_count -= 1;
            if !items.is_empty() {
                write!(formatter, " ")?;
            }
            write!(formatter, "{right}", right = "]".paint(get_bracket_style(*bracket_count)))
        },

        AttributeSet as set => {
            write!(formatter, "{left}", left = "{".paint(get_bracket_style(*bracket_count)))?;
            *bracket_count += 1;

            let inherits: Vec<_> = set.inherits().collect();
            for inherit in inherits.iter() {
                write!(formatter, " ")?;
                s_impl(formatter, &inherit.identifier(), bracket_count)?;
                write!(formatter, ";")?;
            }

            let attributes: Vec<_> = set.attributes().collect();
            for attribute in attributes.iter() {
                write!(formatter, " ")?;

                let mut identifiers = attribute.path().identifiers();
                if let Some(first) = identifiers.next() {
                    s_impl(formatter, &first, bracket_count)?;
                }
                for identifier in identifiers {
                    write!(formatter, ".")?;
                    s_impl(formatter, &identifier, bracket_count)?;
                }

                write!(formatter, " = ")?;
                s_impl(formatter, &attribute.value(), bracket_count)?;
                write!(formatter, ";")?;
            }

            *bracket_count -= 1;
            if !inherits.is_empty() || !attributes.is_empty() {
                write!(formatter, " ")?;
            }
            write!(formatter, "{right}", right = "}".paint(get_bracket_style(*bracket_count)))
        },

        AttributeSelect as select => {
            write!(formatter, "{left}", left = "(".paint(get_bracket_style(*bracket_count)))?;
            *bracket_count += 1;

            write!(formatter, "{delimiter}{content}{delimiter} ", delimiter = "`".green().bold(), content = ".".green())?;
            s_impl(formatter, &select.identifier(), bracket_count)?;
            write!(formatter, " ")?;
            s_impl(formatter, &select.expression(), bracket_count)?;

            *bracket_count -= 1;
            write!(formatter, "{right}", right = ")".paint(get_bracket_style(*bracket_count)))
        },

        AttributeCheck as check => {
            write!(formatter, "{left}", left = "(".paint(get_bracket_style(*bracket_count)))?;
            *bracket_count += 1;

            write!(formatter, "{delimiter}{content}{delimiter} ", delimiter = "`".green().bold(), content = "?".green())?;
            s_impl(formatter, &check.expression(), bracket_count)?;

            for attribute in check.attributes() {
                write!(formatter, " ")?;
                s_impl(formatter, &attribute, bracket_count)?;
            }

            *bracket_count -= 1;
            write!(formatter, "{right}", right = ")".paint(get_bracket_style(*bracket_count)))
        },

        Bind as bind => {
            write!(formatter, "{left}", left = "(".paint(get_bracket_style(*bracket_count)))?;
            *bracket_count += 1;

            write!(formatter, "bind ")?;
            s_impl(formatter, &bind.identifier(), bracket_count)?;
            write!(formatter, " ")?;
            s_impl(formatter, &bind.expression(), bracket_count)?;

            *bracket_count -= 1;
            write!(formatter, "{right}", right = ")".paint(get_bracket_style(*bracket_count)))

        },

        Lambda as lambda => {
            match lambda.parameter() {
                LambdaParameter::LambdaParameterIdentifier(parameter) => s_impl(formatter, &parameter.identifier(), bracket_count)?,
                LambdaParameter::LambdaParameterPattern(parameter) => {
                    write!(formatter, "{left}", left = "{".paint(get_bracket_style(*bracket_count)))?;
                    *bracket_count += 1;

                    let attributes: Vec<_> = parameter.attributes().collect();

                    for (index, attribute) in attributes.iter().enumerate() {
                        s_impl(formatter, &attribute.identifier(), bracket_count)?;

                        if let Some(default) = attribute.default() {
                            write!(formatter, " ? ")?;
                            s_impl(formatter, &default, bracket_count)?;
                        }

                        if index + 1 != attributes.len() {
                            write!(formatter, ", ")?;
                        }
                    }

                    *bracket_count -= 1;
                    if !attributes.is_empty() {
                        write!(formatter, " ")?;
                    }
                    write!(formatter, "{right}", right = "}".paint(get_bracket_style(*bracket_count)))?;
                },
            }

            write!(formatter, ": ")?;
            s_impl(formatter, &lambda.expression(), bracket_count)
        },

        Application as application => {
            write!(formatter, "{left}", left = "(".paint(get_bracket_style(*bracket_count)))?;
            *bracket_count += 1;

            s_impl(formatter, &application.left_expression(), bracket_count)?;
            write!(formatter, " ")?;
            s_impl(formatter, &application.right_expression(), bracket_count)?;

            *bracket_count -= 1;
            write!(formatter, "{right}", right = ")".paint(get_bracket_style(*bracket_count)))
        },

        PrefixOperation as operation => {
            write!(formatter, "{left}", left = "(".paint(get_bracket_style(*bracket_count)))?;
            *bracket_count += 1;

            write!(formatter, "{delimiter}{operator}{delimiter}", delimiter = "`".green().bold(), operator = match operation.operator() {
                PrefixOperator::Swwallation => "+",
                PrefixOperator::Negation => "-",

                PrefixOperator::Not => "not",
            }.green())?;
            write!(formatter, " ")?;
            s_impl(formatter, &operation.expression(), bracket_count)?;

            *bracket_count -= 1;
            write!(formatter, "{right}", right = ")".paint(get_bracket_style(*bracket_count)))
        },

        InfixOperation as operation => {
            write!(formatter, "{left}", left = "(".paint(get_bracket_style(*bracket_count)))?;
            *bracket_count += 1;

            write!(formatter, "{delimiter}{operator}{delimiter}", delimiter = "`".green().bold(), operator = match operation.operator() {
                InfixOperator::Apply => "//",
                InfixOperator::Pipe => "|>",

                InfixOperator::Concat => "++",

                InfixOperator::Use => "==>",
                InfixOperator::Override => "<==",
                InfixOperator::Update => "//",

                InfixOperator::Equal => "==",
                InfixOperator::NotEqual => "!=",
                InfixOperator::LessOrEqual => "<=",
                InfixOperator::Less => "<",
                InfixOperator::MoreOrEqual => ">=",
                InfixOperator::More => ">",
                InfixOperator::Implication => "->",

                InfixOperator::Addition => "+",
                InfixOperator::Subtraction => "-",
                InfixOperator::Multiplication => "*",
                InfixOperator::Power => "**",
                InfixOperator::Division => "/",

                InfixOperator::And => "and",
                InfixOperator::Or => "or",
            }.green())?;
            write!(formatter, " ")?;
            s_impl(formatter, &operation.left_expression(), bracket_count)?;
            write!(formatter, " ")?;
            s_impl(formatter, &operation.right_expression(), bracket_count)?;

            *bracket_count -= 1;
            write!(formatter, "{right}", right = ")".paint(get_bracket_style(*bracket_count)))
        },

        Path as path => s_impl_parted(path.parts(), formatter, bracket_count),

        Identifier as identifier => {
            match identifier.value() {
                IdentifierValue::Simple(token) => write!(formatter, "{identifier}", identifier = match token.text() {
                    boolean @ ("true" | "false") => boolean.magenta().bold(),
                    inexistent @ ("null" | "undefined") => inexistent.cyan().bold(),
                    identifier => identifier.new(),
                }),

                IdentifierValue::Complex(complex) => s_impl_parted(complex.parts(), formatter, bracket_count),
            }
        },

        SString as sstring => s_impl_parted(sstring.parts(), formatter, bracket_count),

        Island as island => s_impl_parted(island.parts(), formatter, bracket_count),

        Number as number => {
            match number.value() {
                NumberValue::Integer(token) => write!(formatter, "{number}", number = token.value().blue().bold()),
                NumberValue::Float(token) => write!(formatter, "{number}", number = token.value().blue().bold()),
            }
        },

        IfElse as if_else => {
            write!(formatter, "{left}", left = "(".paint(get_bracket_style(*bracket_count)))?;
            *bracket_count += 1;

            write!(formatter, "{if} ", r#if = "if-else".red().bold())?;

            s_impl(formatter, &if_else.condition(), bracket_count)?;
            write!(formatter, " ")?;
            s_impl(formatter, &if_else.true_expression(), bracket_count)?;

            if let Some(false_expression) = if_else.false_expression() {
                write!(formatter, " ")?;
                s_impl(formatter, &false_expression, bracket_count)?;
            }

            *bracket_count -= 1;
            write!(formatter, "{right}", right = ")".paint(get_bracket_style(*bracket_count)))
        },

        else => unreachable!(),
    }
}
