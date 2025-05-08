//! Text formatting.

#![feature(gen_blocks, if_let_guard, let_chains, mixed_integer_ops_unsigned_sub)]
#![allow(unstable_name_collisions)] // Itertools::intersperse

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

mod tag;
pub use tag::{
   Tag,
   TagCondition,
   Tags,
};

mod view;
pub use view::{
   DebugView,
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
   pub use super::indent::IndentPlace;
}
