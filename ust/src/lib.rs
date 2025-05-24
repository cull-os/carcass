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
use style::StyledExt;

pub mod report;
pub mod style;

pub mod terminal;

pub trait Display<W: Write> {
   fn display_styled(&self, w: &mut W) -> fmt::Result;
}

trait IntoWidth {
   fn width(&self) -> usize;
}

impl IntoWidth for &'_ str {
   fn width(&self) -> usize {
      terminal::width(self)
   }
}

impl IntoWidth for Cow<'_, str> {
   fn width(&self) -> usize {
      terminal::width(self.as_ref())
   }
}

impl IntoWidth for style::Styled<&'_ str> {
   fn width(&self) -> usize {
      terminal::width(self.value)
   }
}

impl IntoWidth for style::Styled<Cow<'_, str>> {
   fn width(&self) -> usize {
      terminal::width(self.value.as_ref())
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

   fn dedent<'a>(&'a mut self) -> ScopeGuard<&'a mut Self, impl FnOnce(&'a mut Self)>;

   fn indent<'a>(&'a mut self, count: u8) -> ScopeGuard<&'a mut Self, impl FnOnce(&'a mut Self)> {
      self.indent_with(
         count,
         Box::new(move |this| this.write_str(&SPACES[..count as _])),
      )
   }

   fn indent_by<'a>(
      &'a mut self,
      header: style::Styled<&str>,
      continuation: style::Styled<&str>,
   ) -> ScopeGuard<&'a mut Self, impl FnOnce(&'a mut Self)>
   where
      Self: Sized,
   {
      let header = header.value.to_owned().style(header.style);
      let continuation = continuation.value.to_owned().style(continuation.style);

      let mut wrote = false;

      self.indent_with(
         terminal::width(&header.value) as u8,
         Box::new(move |this| {
            if wrote {
               this.write_str(&continuation)
            } else {
               wrote = true;
               this.write_styled(&header)
            }
         }),
      )
   }

   fn indent_header<'a>(
      &'a mut self,
      header: style::Styled<&str>,
   ) -> ScopeGuard<&'a mut Self, impl FnOnce(&'a mut Self)>
   where
      Self: Sized,
   {
      let continuation = SPACES[..header.width()].styled();

      self.indent_by(header, continuation)
   }

   fn indent_with<'a>(
      &'a mut self,
      count: u8,
      with: IndentWith<Self>,
   ) -> ScopeGuard<&'a mut Self, impl FnOnce(&'a mut Self)>;

   fn write_style(&mut self, style: style::Style) -> fmt::Result;

   fn write_styled<T: fmt::Display>(&mut self, styled: &style::Styled<T>) -> fmt::Result;

   fn write_report(
      &mut self,
      report: &report::Report,
      location: style::Styled<&str>,
      source: &report::PositionStr<'_>,
   ) -> fmt::Result
   where
      Self: Sized;
}
