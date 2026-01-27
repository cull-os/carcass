#![feature(anonymous_lifetime_in_impl_trait, gen_blocks, if_let_guard)]
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

pub fn display(display: &impl fmt::Display) -> impl Display {
   struct FmtDisplay<'a, D: fmt::Display>(&'a D);

   impl<D: fmt::Display> Display for FmtDisplay<'_, D> {
      fn display_styled(&self, writer: &mut dyn Write) -> fmt::Result {
         write!(writer, "{inner}", inner = self.0)
      }
   }

   FmtDisplay(display)
}

pub fn with<W: Write + ?Sized, T>(
   writer: &mut W,
   style: style::Style,
   with: impl FnOnce(&mut W) -> T,
) -> T {
   let style_previous = writer.get_style();

   writer.set_style(style);
   let result = with(writer);

   writer.set_style(style_previous);
   result
}

pub fn write(writer: &mut dyn Write, styled: &impl Display) -> fmt::Result {
   styled.display_styled(writer)
}

pub trait Write: fmt::Write {
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
