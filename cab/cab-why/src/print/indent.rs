use std::{
   fmt,
   sync::atomic,
};

use crate::text::LINE_WIDTH;

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
pub type IndentWith<'a> = &'a mut dyn FnMut(&mut dyn fmt::Write) -> Result<u16, fmt::Error>;

/// An indent writer.
///
/// TODO: Explain how it behaves properly.
pub struct IndentWriter<'a> {
   #[doc(hidden)]
   pub __writer: &'a mut dyn fmt::Write,
   #[doc(hidden)]
   pub __with:   IndentWith<'a>,
   #[doc(hidden)]
   pub __count:  u16,
   #[doc(hidden)]
   pub __place:  IndentPlace,
}

impl Drop for IndentWriter<'_> {
   fn drop(&mut self) {
      let width = LINE_WIDTH.load(atomic::Ordering::Acquire);
      LINE_WIDTH.store(
         width.saturating_sub(self.__count),
         atomic::Ordering::Release,
      );
   }
}

impl fmt::Write for IndentWriter<'_> {
   fn write_str(&mut self, s: &str) -> fmt::Result {
      use None as New;
      use Some as Line;

      for line in s.split('\n').map(Line).intersperse(New) {
         match self.__place {
            IndentPlace::Start
               if let Line(line) = line
                  && !line.is_empty() =>
            {
               self.write_indent()?;
            },

            IndentPlace::End => {
               writeln!(self.__writer)?;
               self.__place = IndentPlace::Start;
            },

            _ => {},
         }

         match line {
            New => self.__place = IndentPlace::End,

            Line(line) => {
               write!(self.__writer, "{line}")?;
            },
         }
      }

      Ok(())
   }
}

impl IndentWriter<'_> {
   /// Asserts that it is at the start of the line and writes the indent.
   pub fn write_indent(&mut self) -> fmt::Result {
      assert_eq!(self.__place, IndentPlace::Start);

      let wrote = (self.__with)(self.__writer)?;

      if wrote > self.__count {
         panic!(
            "indent writer wrote ({wrote}) more than the indent ({count})",
            count = self.__count
         );
      }

      write!(
         self.__writer,
         "{:>count$}",
         "",
         count = (self.__count - wrote) as usize
      )?;
      self.__place = IndentPlace::Middle;

      Ok(())
   }
}

/// Creates an [`IndentWriter`] with the given [`fmt::Write`] and indent count.
pub fn indent(writer: &mut dyn fmt::Write, count: u16) -> IndentWriter<'_> {
   static mut ZERO_INDENTER: IndentWith<'static> = &mut |_| Ok(0);

   LINE_WIDTH.fetch_add(count, atomic::Ordering::Acquire);

   IndentWriter {
      __writer: writer,
      // SAFETY: ZERO_INDENTER does not modify anything and the pointee of self.writer in Writer
      // is never replaced. Therefore we can use it, because without writes you can't have
      // race conditions.
      __with:   unsafe { ZERO_INDENTER },
      __count:  count,
      __place:  IndentPlace::Start,
   }
}

/// Creates an [`IndentWriter`] with the given [`fmt::Write`], indent count and
/// [`IndentWith`].
///
/// Consult the documentation on [`IndentWith`] to learn what it is used for.
pub fn indent_with<'a>(
   writer: &'a mut dyn fmt::Write,
   count: u16,
   with: IndentWith<'a>,
) -> IndentWriter<'a> {
   LINE_WIDTH.fetch_add(count, atomic::Ordering::Acquire);

   IndentWriter {
      __writer: writer,
      __with:   with,
      __count:  count,
      __place:  IndentPlace::Start,
   }
}

#[macro_export]
macro_rules! indent {
   ($writer:ident,header = $header:expr) => {
      let header = $header;

      let header_width: u16 = {
         use $crate::__private::unicode_width::UnicodeWidthStr as _;

         trait ToStr {
            fn to_str(&self) -> &str;
         }

         impl ToStr for &'_ str {
            fn to_str(&self) -> &str {
               self
            }
         }

         impl ToStr for ::std::borrow::Cow<'_, str> {
            fn to_str(&self) -> &str {
               self.as_ref()
            }
         }

         impl ToStr for ::yansi::Painted<&'_ str> {
            fn to_str(&self) -> &str {
               self.value
            }
         }

         impl ToStr for ::yansi::Painted<::std::borrow::Cow<'_, str>> {
            fn to_str(&self) -> &str {
               self.value.as_ref()
            }
         }

         header
            .to_str()
            .width()
            .try_into()
            .expect("header width must be smaller than u16::MAX")
      };

      let mut wrote = false;
      $crate::indent!(
         $writer,
         header_width + 1,
         with = move |writer: &mut dyn ::std::fmt::Write| {
            if wrote {
               return Ok(0);
            }

            write!(writer, "{header} ")?;

            wrote = true;
            Ok(header_width + 1)
         }
      );
   };

   ($writer:ident, $count:expr) => {
      let $writer = &mut $crate::indent($writer, $count);
   };

   ($writer:ident, $count:expr,with = $with:expr) => {
      let mut with = |writer: &mut dyn ::std::fmt::Write| $with(writer).map(|wrote| wrote as u16);

      let $writer = &mut $crate::indent_with($writer, ($count) as u16, &mut with);
   };
}

#[macro_export]
macro_rules! dedent {
   ($writer:ident) => {
      $crate::dedent!($writer, $writer.__count, discard = true);
   };

   ($writer:ident, $dedent:expr) => {
      $crate::dedent!($writer, $dedent, discard = true);
   };

   ($writer:ident, $dedent:expr,discard = $discard:literal) => {
      use std::sync::atomic;

      use $crate::__private::{
         LINE_WIDTH,
         scopeguard,
      };

      let dedent = $dedent as u16;

      let old_count = LINE_WIDTH.load(atomic::Ordering::Acquire);
      LINE_WIDTH.store(old_count.saturating_sub(dedent), atomic::Ordering::Release);

      let _guard = scopeguard::guard((), |_| {
         LINE_WIDTH.store(old_count, atomic::Ordering::Release);
      });

      let $writer = &mut $crate::IndentWriter {
         __writer: $writer.__writer,
         __count:  $writer
            .__count
            .checked_sub(dedent)
            .expect("dedent must be smaller than indent"),
         __with:   if $discard {
            &mut move |_| Ok(0)
         } else {
            $writer.__with
         },
         __place:  $writer.__place,
      };
   };
}
