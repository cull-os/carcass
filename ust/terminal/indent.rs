use std::fmt;

use itertools::Itertools as _;

use crate::{
   Display,
   Write,
   report,
   style,
   style::StyledExt as _,
   write,
};

#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
   Start,
   Middle,
   End,
}

pub struct Writer<'a> {
   #[doc(hidden)]
   pub inner: &'a mut dyn Write,
   #[doc(hidden)]
   pub with:  &'a mut dyn FnMut(&mut dyn Write) -> Result<usize, fmt::Error>,
   #[doc(hidden)]
   pub count: usize,
   #[doc(hidden)]
   pub place: Place,
}

impl Write for Writer<'_> {
   fn finish(&mut self) -> fmt::Result {
      self.inner.finish()
   }

   fn width(&self) -> usize {
      self.inner.width()
         + if self.place == Place::Start {
            self.count
         } else {
            0
         }
   }

   fn width_max(&self) -> usize {
      self.inner.width_max()
   }

   fn get_style(&self) -> style::Style {
      self.inner.get_style()
   }

   fn set_style(&mut self, style: style::Style) {
      self.inner.set_style(style);
   }

   fn apply_style(&mut self) -> fmt::Result {
      self.inner.apply_style()
   }

   fn write_report(
      &mut self,
      report: &report::Report,
      location: &dyn Display,
      source: &report::PositionStr<'_>,
   ) -> fmt::Result
   where
      Self: Sized,
   {
      super::write_report(self, report, location, source)
   }
}

impl Writer<'_> {
   /// Asserts that it is at the start of the line and writes the indent.
   ///
   /// # Panics
   ///
   /// Panics if the writer isn't at the start of the line or if the indent
   /// writer wrote more than the indent.
   #[track_caller]
   pub fn write_indent(&mut self) -> fmt::Result {
      assert_eq!(self.place, Place::Start);

      let wrote = (self.with)(self.inner)?;

      assert!(
         wrote <= self.count,
         "indent writer wrote ({wrote}) more than the indent ({count})",
         count = self.count
      );

      for _ in wrote..self.count {
         write(self.inner, &' '.style(style::Style::default()))?;
      }

      self.place = Place::Middle;

      Ok(())
   }
}

impl fmt::Write for Writer<'_> {
   fn write_str(&mut self, s: &str) -> fmt::Result {
      use None as Newline;
      use Some as Line;

      for segment in s.split('\n').map(Line).intersperse(Newline) {
         match self.place {
            Place::Start
               if let Line(line) = segment
                  && !line.is_empty() =>
            {
               self.write_indent()?;
            },

            Place::End => {
               self.inner.write_char('\n')?;
               self.place = Place::Start;
            },

            Place::Start | Place::Middle => {},
         }

         match segment {
            Newline => self.place = Place::End,

            Line(line) => {
               self.inner.write_str(line)?;
            },
         }
      }

      Ok(())
   }
}

pub use crate::__indent as indent;

#[doc(hidden)]
#[macro_export]
macro_rules! __indent {
   ($writer:ident,header = $header:expr $(,)?) => {
      $crate::terminal::indent!($writer, header = $header, continuation = "".styled())
   };

   ($writer:ident,header = $header:expr,continuation = $continuation:expr $(,)?) => {
      let header = $header;
      let continuation = $continuation;

      let (header_width, continuation_width) = {
         trait CastStr {
            fn cast_str(&self) -> &str;
         }

         impl CastStr for &'_ str {
            fn cast_str(&self) -> &str {
               self
            }
         }

         impl CastStr for std::borrow::Cow<'_, str> {
            fn cast_str(&self) -> &str {
               self.as_ref()
            }
         }

         impl CastStr for $crate::style::Styled<&'_ str> {
            fn cast_str(&self) -> &str {
               self.value
            }
         }

         impl CastStr for $crate::style::Styled<std::borrow::Cow<'_, str>> {
            fn cast_str(&self) -> &str {
               self.value.as_ref()
            }
         }

         (
            $crate::terminal::width(header.cast_str()),
            $crate::terminal::width(continuation.cast_str()),
         )
      };

      let mut wrote = false;
      $crate::terminal::indent!($writer, header_width + 1, |writer| {
         if !wrote {
            $crate::write(writer, &header)?;

            wrote = true;
            Ok(header_width)
         } else {
            $crate::write(writer, &continuation)?;

            Ok(continuation_width)
         }
      });
   };

   ($writer:ident, $count:expr $(,)?) => {
      $crate::terminal::indent!($writer, $count, |_| Ok(0_usize));
   };

   ($writer:ident, $count:expr, $with:expr $(,)?) => {
      let $writer = &mut $crate::terminal::indent::Writer {
         inner: $writer,
         with:  &mut $with,
         count: $count,
         place: $crate::terminal::indent::Place::Start,
      };
   };
}

pub use crate::__dedent as dedent;

#[doc(hidden)]
#[macro_export]
macro_rules! __dedent {
   ($writer:ident $(,)?) => {
      $crate::terminal::dedent!($writer, $writer.count, discard = true);
   };

   ($writer:ident, $dedent:expr $(,)?) => {
      $crate::terminal::dedent!($writer, $dedent, discard = true);
   };

   ($writer:ident, $dedent:expr,discard = $discard:literal $(,)?) => {
      let $writer = &mut $crate::terminal::indent::Writer {
         inner: $writer.inner,

         count: $writer
            .count
            .checked_sub($dedent)
            .expect("dedent must be smaller than indent"),

         with: if $discard {
            &mut move |_| Ok(0)
         } else {
            $writer.with
         },

         place: $writer.place,
      };
   };
}
