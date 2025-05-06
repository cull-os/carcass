//! Text formatting.

#![feature(iter_intersperse, if_let_guard, let_chains)]

mod color;
pub use color::COLORS;

mod indent;
pub use indent::{
   IndentWith,
   IndentWriter,
   indent,
   indent_with,
};

pub mod style;

#[path = "width.rs"] mod width_;
pub use width_::{
   number_hex_width,
   number_width,
   width,
};

mod wrap;
pub use wrap::{
   lnwrap,
   wrap,
};

/// Initialize data required to format text.
pub fn init() {
   style::init();
}

#[doc(hidden)]
pub mod private {
   use std::sync;

   pub use scopeguard::guard;

   pub use super::indent::IndentPlace;

   pub static LINE_WIDTH: sync::atomic::AtomicUsize = sync::atomic::AtomicUsize::new(0);

   pub static LINE_WIDTH_MAX: sync::LazyLock<usize> = sync::LazyLock::new(|| {
      terminal_size::terminal_size().map_or(100, |(width, _)| width.0 as usize)
   });
}
