//! Text formatting.

#![feature(iter_intersperse, if_let_guard, let_chains)]

mod color;

pub use color::COLORS;

mod indent;
pub use indent::{
   IndentWith,
   IndentWriter,
};

pub mod style;

#[path = "width.rs"] mod width_;
pub use width_::{
   number_hex_width,
   number_width,
   width,
};

mod view;
pub use view::{
   DisplayView,
   View,
   WriteView,
   stderr,
   stdout,
};

mod wrap;
pub use wrap::{
   lnwrap,
   wrap,
};

#[doc(hidden)]
pub fn init() {
   style::init();
}

#[doc(hidden)]
pub mod private {
   pub use scopeguard::guard;

   pub use super::indent::IndentPlace;
}
