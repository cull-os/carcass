#![feature(
   gen_blocks,
   if_let_guard,
   iter_intersperse,
   anonymous_lifetime_in_impl_trait
)]

use std::fmt;

pub mod report;
pub mod style;

pub mod terminal;

pub trait Display {
   fn display_styled(&self, w: &mut dyn Write) -> fmt::Result;
}

pub fn write(writer: &mut dyn Write, styled: &style::Styled<impl fmt::Display>) -> fmt::Result {
   let style_previous = writer.get_style();

   writer.set_style(styled.style);
   write!(writer, "{value}", value = **styled)?;

   writer.set_style(style_previous);
   Ok(())
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
