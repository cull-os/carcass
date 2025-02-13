#![feature(if_let_guard, let_chains, iter_intersperse)]

mod indent;
mod wrap;

use std::{
    cell::Cell,
    sync::LazyLock,
};

pub use crate::{
    indent::*,
    wrap::*,
};

#[doc(hidden)]
pub mod macro_crates {
    pub use scopeguard;
    pub use unicode_width;
    pub use yansi;
}

static LINE_WIDTH_MAX: LazyLock<u16> =
    LazyLock::new(|| terminal_size::terminal_size().map(|(width, _)| width.0).unwrap_or(120));

thread_local! {
    #[doc(hidden)]
    pub static LINE_WIDTH: Cell<u16> = const { Cell::new(0) };
}
