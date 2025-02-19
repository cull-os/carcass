#![feature(if_let_guard, let_chains, iter_intersperse)]

mod indent;
mod size;
mod span;
mod wrap;

use std::sync::LazyLock;

pub use crate::{
    indent::*,
    size::*,
    span::*,
    wrap::*,
};

#[doc(hidden)]
pub mod __private {
    use std::cell::Cell;

    pub use scopeguard;
    pub use unicode_width;
    pub use yansi;

    thread_local! {
        pub static LINE_WIDTH: Cell<u16> = const { Cell::new(0) };
    }
}

static LINE_WIDTH_MAX: LazyLock<u16> =
    LazyLock::new(|| terminal_size::terminal_size().map(|(width, _)| width.0).unwrap_or(120));

// These two are kind of random but I'll include them here anyway.
// This crate is used everywhere.

#[macro_export]
macro_rules! into {
    ($($variable:ident),*) => {
        $(
            let $variable = $variable.into();
        )*
    }
}

pub fn paint<T>(value: T, style: yansi::Style) -> yansi::Painted<T> {
    use yansi::Paint as _;

    let mut painted = value.new();
    painted.style = style;
    painted
}
