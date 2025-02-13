use std::fmt;

use crate::LINE_WIDTH;

#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndentPlace {
    Start,
    Middle,
    End,
}

type IndentWith<'a> = &'a mut dyn FnMut(&mut dyn fmt::Write) -> Result<u16, fmt::Error>;

pub struct IndentWriter<'a> {
    #[doc(hidden)]
    pub writer: &'a mut dyn fmt::Write,
    #[doc(hidden)]
    pub with: IndentWith<'a>,
    #[doc(hidden)]
    pub count: u16,
    #[doc(hidden)]
    pub place: IndentPlace,
}

impl Drop for IndentWriter<'_> {
    fn drop(&mut self) {
        LINE_WIDTH.set(LINE_WIDTH.get().saturating_sub(self.count));
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

                _ => {},
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
    pub fn write_indent(&mut self) -> fmt::Result {
        assert_eq!(self.place, IndentPlace::Start);

        let wrote = (self.with)(self.writer)?;

        if wrote > self.count {
            panic!(
                "indent writer wrote ({wrote}) more than the indent ({count})",
                count = self.count
            );
        }

        write!(self.writer, "{:>count$}", "", count = (self.count - wrote) as usize)?;
        self.place = IndentPlace::Middle;

        Ok(())
    }
}

pub fn indent(writer: &mut dyn fmt::Write, count: u16) -> IndentWriter<'_> {
    static mut ZERO_INDENTER: IndentWith<'static> = &mut |_| Ok(0);

    LINE_WIDTH.set(LINE_WIDTH.get() + count);

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

pub fn indent_with<'a>(writer: &'a mut dyn fmt::Write, count: u16, with: IndentWith<'a>) -> IndentWriter<'a> {
    LINE_WIDTH.set(LINE_WIDTH.get() + count);

    IndentWriter {
        writer,
        with,
        count,
        place: IndentPlace::Start,
    }
}

#[macro_export]
macro_rules! indent {
    ($writer:ident,header = $header:expr) => {
        let header_width: u16 = {
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

            let header = $header;
            $crate::macro_crates::unicode_width::UnicodeWidthStr::width(header.to_str())
                .try_into()
                .expect("header too big")
        };

        let mut wrote = false;
        $crate::indent!(
            $writer,
            header_width + 1,
            with = move |writer: &mut dyn ::std::fmt::Write| {
                if wrote {
                    return Ok(0);
                }

                write!(writer, "{header} ", header = $header)?;

                wrote = true;
                Ok(header_width + 1)
            }
        );
    };

    ($writer:ident, $count:expr) => {
        let $writer = &mut $crate::indent($writer, $count);
    };

    ($writer:ident, $count:expr,with = $with:expr) => {
        let mut with = $with;
        let mut with = |writer: &mut dyn ::std::fmt::Write| with(writer).map(|wrote| wrote as u16);

        let $writer = &mut $crate::indent_with($writer, ($count) as u16, &mut with);
    };
}

#[macro_export]
macro_rules! dedent {
    ($writer:ident) => {
        $crate::dedent!($writer, $writer.count, discard = true);
    };

    ($writer:ident, $dedent:expr) => {
        $crate::dedent!($writer, $dedent, discard = true);
    };

    ($writer:ident, $dedent:expr,discard = $discard:literal) => {
        let dedent = ($dedent) as u16;
        let old_count = $crate::LINE_WIDTH.get();

        $crate::LINE_WIDTH.set(old_count.saturating_sub(dedent));
        let _guard = $crate::macro_crates::scopeguard::guard((), |_| {
            $crate::LINE_WIDTH.set(old_count);
        });

        let $writer = &mut $crate::IndentWriter {
            writer: $writer.writer,
            count: $writer.count.checked_sub(dedent).expect("dedent was more than indent"),
            with: if $discard { &mut move |_| Ok(0) } else { $writer.with },
            place: $writer.place,
        };
    };
}
