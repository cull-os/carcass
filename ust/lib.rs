#![feature(
   anonymous_lifetime_in_impl_trait,
   gen_blocks,
   if_let_guard,
   mixed_integer_ops_unsigned_sub
)]
#![allow(unstable_name_collisions)] // Itertools::intersperse

use std::fmt;

mod color;
pub use color::{
   COLORS,
   STYLE_GUTTER,
   STYLE_HEADER_POSITION,
};

pub mod report;
pub mod style;

pub mod terminal;

pub const INDENT: &str = "   ";
#[expect(clippy::cast_possible_wrap)]
pub const INDENT_WIDTH: isize = INDENT.len() as isize;

pub trait Debug {
   fn debug_styled(&self, writer: &mut dyn Write) -> fmt::Result;
}

pub trait Display {
   fn display_styled(&self, writer: &mut dyn Write) -> fmt::Result;
}

pub fn with<T>(
   writer: &mut dyn Write,
   style: style::Style,
   closure: impl FnOnce(&mut dyn Write) -> T,
) -> T {
   let style_previous = writer.get_style();

   writer.set_style(style);
   let result = closure(writer);

   writer.set_style(style_previous);
   result
}

pub fn write(writer: &mut dyn Write, styled: &style::Styled<impl fmt::Display>) -> fmt::Result {
   with(writer, styled.style, |writer| {
      write!(writer, "{value}", value = **styled)
   })
}

pub trait Write: fmt::Write {
   fn finish(&mut self) -> fmt::Result {
      Ok(())
   }

   fn width(&self) -> usize {
      0
   }

   fn width_max(&self) -> usize {
      usize::MAX
   }

   fn get_style(&self) -> style::Style;

   fn set_style(&mut self, style: style::Style);

   fn apply_style(&mut self) -> fmt::Result;

   fn write_report(
      &mut self,
      report: &report::Report,
      location: &dyn Display,
      source: &report::PositionStr<'_>,
   ) -> fmt::Result;
}
