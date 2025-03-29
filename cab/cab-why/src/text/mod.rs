mod size;
mod span;

use std::sync::{
    LazyLock,
    atomic,
};

pub use self::{
    size::{
        IntoSize,
        Size,
    },
    span::{
        IntoSpan,
        Span,
    },
};

pub static LINE_WIDTH: atomic::AtomicU16 = atomic::AtomicU16::new(0);

pub static LINE_WIDTH_MAX: LazyLock<u16> = LazyLock::new(|| {
    let width = terminal_size::terminal_size().map(|(width, _)| width.0);

    width.unwrap_or(120)
});
