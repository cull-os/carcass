//! Text and report formatting utilities and data types.

#![feature(gen_blocks, if_let_guard, iter_intersperse, let_chains, trait_alias, try_trait_v2)]

mod error;
mod print;
mod report;
mod text;

pub use self::{
    error::*,
    print::{
        IndentWith,
        IndentWriter,
        indent,
        indent_with,
        wrap,
        wrapln,
    },
    report::*,
    text::*,
};

/// A macro to make mass redeclarations of a collection of identifiers using a
/// single method more concise.
///
/// # Example
///
/// ```rs
/// // This:
/// call!(foo; bar, qux);
///
/// // Gets turned into this:
/// let bar = bar.foo();
/// let qux = qux.foo();
/// ```
#[macro_export]
macro_rules! call {
    ($method:ident; $($variable:ident),*) => {
        $(let $variable = $variable.$method();)*
    }
}

/// [`call!`] but with the method set to `as_ref`.
#[macro_export]
macro_rules! as_ref {
    ($($t:tt),*) => {
        $crate::call!(as_ref; $($t),*);
    }
}

/// [`call!`] but with the method set to `clone`.
#[macro_export]
macro_rules! clone {
    ($($t:tt),*) => {
        $crate::call!(clone; $($t),*);
    }
}

/// [`call!`] but with the method set to `into`.
#[macro_export]
macro_rules! into {
    ($($t:tt),*) => {
        $crate::call!(into; $($t),*);
    }
}

#[doc(hidden)]
pub mod __private {
    use std::sync::{
        LazyLock,
        atomic,
    };

    pub use anyhow;
    pub use scopeguard;
    pub use unicode_width;
    pub use yansi;

    pub use crate::print::IndentPlace;

    static LINE_WIDTH: atomic::AtomicU16 = atomic::AtomicU16::new(0);

    pub fn line_width_load() -> u16 {
        LINE_WIDTH.load(atomic::Ordering::SeqCst)
    }

    pub fn line_width_store(value: u16) {
        LINE_WIDTH.store(value, atomic::Ordering::SeqCst);
    }

    pub static LINE_WIDTH_MAX: LazyLock<u16> = LazyLock::new(|| {
        let width = terminal_size::terminal_size().map(|(width, _)| width.0);

        width.unwrap_or(120)
    });
}
