#![feature(f128)]

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
