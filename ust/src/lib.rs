#![feature(
   gen_blocks,
   if_let_guard,
   iter_intersperse,
   anonymous_lifetime_in_impl_trait
)]

use std::{
   borrow::Cow,
   fmt,
};

use scopeguard::ScopeGuard;

pub mod report;
pub mod style;

pub mod terminal;

pub trait Display<'a, W: Write> {
   fn display_styled(&self, w: &mut W) -> fmt::Result;
}

trait ToStr {
   fn to_str(&self) -> &str;
}

impl ToStr for &'_ str {
   fn to_str(&self) -> &str {
      self
   }
}

impl ToStr for Cow<'_, str> {
   fn to_str(&self) -> &str {
      self.as_ref()
   }
}

impl ToStr for style::Styled<&'_ str> {
   fn to_str(&self) -> &str {
      self.value
   }
}

impl ToStr for style::Styled<Cow<'_, str>> {
   fn to_str(&self) -> &str {
      self.value.as_ref()
   }
}

const SPACES: &str = const {
   if let Ok(indents) = str::from_utf8(&[b' '; u8::MAX as _]) {
      indents
   } else {
      unreachable!()
   }
};

pub type IndentWith<W: Write> = Box<dyn FnMut(&mut W) -> fmt::Result>;

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

   fn dedent(&mut self);

   fn indent<'a>(&'a mut self, count: u8) -> ScopeGuard<&'a mut Self, impl FnOnce(&'a mut Self)> {
      self.indent_with(Box::new(move |this| this.write_str(&SPACES[..count as _])))
   }

   fn indent_with<'a>(
      &'a mut self,
      with: IndentWith<Self>,
   ) -> ScopeGuard<&'a mut Self, impl FnOnce(&'a mut Self)>;

   fn write_style(&mut self, style: style::Style) -> fmt::Result;

   fn write_styled<T: fmt::Display>(&mut self, styled: &style::Styled<T>) -> fmt::Result;

   fn write_report(
      &mut self,
      report: &report::Report,
      location: &impl Display<'_, Self>,
      source: &report::PositionStr<'_>,
   ) -> fmt::Result
   where
      Self: Sized;
}
