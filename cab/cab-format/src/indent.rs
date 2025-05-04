use std::{
   fmt,
   sync::atomic,
};

use crate::private::LINE_WIDTH;

#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndentPlace {
   Start,
   Middle,
   End,
}

/// The type that is accepted by [`indent_with`] to print indent prefixes.
///
/// Returns a number, which is the amount of spaces (indents) it has written.
/// If the number is smaller than the [`IndentWriter`] count, the diff
/// will be printed as spaces.
///
/// If it is higher than that number, the [`IndentWriter`] will panic.
pub type IndentWith<'scope> =
   &'scope mut dyn FnMut(&mut dyn fmt::Write) -> Result<usize, fmt::Error>;

/// An indent writer.
///
/// TODO: Explain how it behaves properly.
pub struct IndentWriter<'scope> {
   #[doc(hidden)]
   pub writer: &'scope mut dyn fmt::Write,
   #[doc(hidden)]
   pub with:   IndentWith<'scope>,
   #[doc(hidden)]
   pub count:  usize,
   #[doc(hidden)]
   pub place:  IndentPlace,
}

impl Drop for IndentWriter<'_> {
   fn drop(&mut self) {
      let width = LINE_WIDTH.load(atomic::Ordering::SeqCst);
      LINE_WIDTH.store(width.saturating_sub(self.count), atomic::Ordering::SeqCst);
   }
}

impl fmt::Write for IndentWriter<'_> {
   fn write_str(&mut self, s: &str) -> fmt::Result {
      use None as New;
      use Some as Line;

      for line in s.split('\n').map(Line).intersperse(New) {
         match self.place {
            IndentPlace::Start
               if let Line(line) = line
                  && !line.is_empty() =>
            {
               self.write_indent()?;
            },

            IndentPlace::End => {
               writeln!(self.writer)?;
               self.place = IndentPlace::Start;
            },

            IndentPlace::Start | IndentPlace::Middle => {},
         }

         match line {
            New => self.place = IndentPlace::End,

            Line(line) => {
               write!(self.writer, "{line}")?;
            },
         }
      }

      Ok(())
   }
}

impl IndentWriter<'_> {
   /// Asserts that it is at the start of the line and writes the indent.
   ///
   /// # Panics
   ///
   /// Panics if the writer isn't at the start of the line or if the indent
   /// writer wrote more than the indent.
   #[track_caller]
   pub fn write_indent(&mut self) -> fmt::Result {
      assert_eq!(self.place, IndentPlace::Start);

      let wrote = (self.with)(self.writer)?;

      assert!(
         wrote <= self.count,
         "indent writer wrote ({wrote}) more than the indent ({count})",
         count = self.count
      );

      write!(self.writer, "{:>count$}", "", count = self.count - wrote)?;
      self.place = IndentPlace::Middle;

      Ok(())
   }
}

/// Creates an [`IndentWriter`] with the given [`fmt::Write`] and indent count.
pub fn indent(writer: &mut dyn fmt::Write, count: usize) -> IndentWriter<'_> {
   static mut ZERO_INDENTER: IndentWith<'static> = &mut |_| Ok(0);

   LINE_WIDTH.fetch_add(count, atomic::Ordering::SeqCst);

   IndentWriter {
      writer,
      // SAFETY: ZERO_INDENTER does not modify anything and the pointee of self.writer in Writer
      // is never replaced. Therefore we can use it, because without writes you can't have
      // race conditions.
      with: unsafe { ZERO_INDENTER },
      count,
      place: IndentPlace::Start,
   }
}

/// Creates an [`IndentWriter`] with the given [`fmt::Write`], indent count and
/// [`IndentWith`].
///
/// Consult the documentation on [`IndentWith`] to learn what it is used for.
pub fn indent_with<'scope>(
   writer: &'scope mut dyn fmt::Write,
   count: usize,
   with: IndentWith<'scope>,
) -> IndentWriter<'scope> {
   LINE_WIDTH.fetch_add(count, atomic::Ordering::SeqCst);

   IndentWriter {
      writer,
      with,
      count,
      place: IndentPlace::Start,
   }
}

#[macro_export]
macro_rules! indent {
   ($writer:ident,header = $header:expr $(,)?) => {
      $crate::indent!($writer, header = $header, continuation = "")
   };

   ($writer:ident,header = $header:expr,continuation = $continuation:expr $(,)?) => {
      let header = $header;
      let continuation = $continuation;

      let (header_width, continuation_width) = {
         trait AsStr {
            fn as_str2(&self) -> &str;
         }

         impl AsStr for &'_ str {
            fn as_str2(&self) -> &str {
               self
            }
         }

         impl AsStr for std::borrow::Cow<'_, str> {
            fn as_str2(&self) -> &str {
               self.as_ref()
            }
         }

         impl AsStr for $crate::style::Styled<&'_ str> {
            fn as_str2(&self) -> &str {
               self.value
            }
         }

         impl AsStr for $crate::style::Styled<std::borrow::Cow<'_, str>> {
            fn as_str2(&self) -> &str {
               self.value.as_ref()
            }
         }

         (
            $crate::width(header.as_str2()),
            $crate::width(continuation.as_str2()),
         )
      };

      let mut wrote = false;
      indent!(
         $writer,
         header_width + 1,
         with = |writer: &mut dyn std::fmt::Write| {
            if !wrote {
               write!(writer, "{header}")?;

               wrote = true;
               Ok(header_width)
            } else {
               write!(writer, "{continuation}")?;

               Ok(continuation_width)
            }
         }
      );
   };

   ($writer:ident, $count:expr) => {
      let $writer = &mut $crate::indent($writer, $count);
   };

   ($writer:ident, $count:expr,with = $with:expr) => {
      let with = &mut $with;

      let $writer = &mut $crate::indent_with($writer, $count, with);
   };
}

#[macro_export]
macro_rules! dedent {
   ($writer:ident) => {
      let writer: &mut $crate::IndentWriter<'_> = $writer;

      $crate::dedent!($writer, writer.count, discard = true);
   };

   ($writer:ident, $dedent:expr) => {
      $crate::dedent!($writer, $dedent, discard = true);
   };

   ($writer:ident, $dedent:expr,discard = $discard:literal) => {
      let dedent: usize = $dedent;
      let discard: bool = $discard;

      let old_count = $crate::private::LINE_WIDTH.load(std::sync::atomic::Ordering::SeqCst);
      $crate::private::LINE_WIDTH.store(
         old_count.saturating_sub(dedent),
         std::sync::atomic::Ordering::SeqCst,
      );

      let _guard = $crate::private::guard((), |_| {
         $crate::private::LINE_WIDTH.store(old_count, std::sync::atomic::Ordering::SeqCst);
      });

      let $writer = &mut $crate::IndentWriter {
         writer: $writer.writer,

         count: $writer
            .count
            .checked_sub(dedent)
            .expect("dedent must be smaller than indent"),

         with: if discard {
            &mut move |_| Ok(0)
         } else {
            $writer.with
         },

         place: $writer.place,
      };
   };
}
