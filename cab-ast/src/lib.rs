#![feature(f128)]

use std::fmt::{
    self,
    Write,
};

use either::Either;
use indexmap::IndexMap;

pub type InterpString = Vec<Either<String, Expr>>;

#[derive(Debug)]
pub enum Expr {
    // with <expr> in <expr>
    WithIn {
        // overrideable with <expr> in <expr>
        overrideable: bool,

        left: Box<Expr>,
        right: Box<Expr>,
    },

    Attrs {
        recursive: bool,
        pairs: IndexMap<Expr, Expr>,
    },

    List(Vec<Expr>),

    // "foo bar ${<expr>} biz"
    String(InterpString),

    // <github:RGBCube/${<expr>}>
    Island(InterpString),

    // ./foo${<expr}/bar/baz.txt
    Path(Vec<InterpString>),

    Number(f128),

    True,
    False,
    Null,
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WithIn {
                overrideable,
                left,
                right,
            } => {
                write!(
                    f,
                    "(with-{}in {left} {right})",
                    if *overrideable { "overrideable-" } else { "" }
                )?;
            },

            Self::Attrs { recursive, pairs } => {
                if *recursive {
                    f.write_str("rec ")?;
                }
                f.write_str("{ ")?;

                for (key, value) in pairs.into_iter() {
                    write!(f, "{key} = {value}; ")?;
                }

                f.write_char('}')?;
            },

            Self::List(exprs) => {
                f.write_str("[ ")?;

                for expr in exprs {
                    write!(f, "{expr} ")?;
                }

                f.write_char(']')?;
            },

            Self::String(parts) => {
                f.write_char('"')?;

                for part in parts {
                    match part {
                        // TODO: Might need to escape some stuff.
                        Either::Left(literal) => f.write_str(literal)?,
                        Either::Right(expr) => {
                            f.write_str("${")?;
                            write!(f, "{expr}")?;
                            f.write_str("}")?;
                        },
                    }
                }

                f.write_char('"')?;
            },

            Self::Number(number) => write!(f, "{number:?}")?,

            Self::True => f.write_str("true")?,
            Self::False => f.write_str("false")?,
            Self::Null => f.write_str("null")?,

            _ => todo!(),
        }

        Ok(())
    }
}
