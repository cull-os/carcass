use std::fmt;

#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    Start,
    Middle,
    End,
}

type IndentWith<'a> = &'a mut dyn FnMut(&mut dyn fmt::Write) -> Result<usize, fmt::Error>;

pub struct Writer<'a> {
    #[doc(hidden)]
    pub writer: &'a mut dyn fmt::Write,
    #[doc(hidden)]
    pub with: IndentWith<'a>,
    #[doc(hidden)]
    pub count: usize,
    #[doc(hidden)]
    pub place: Place,
}

impl Writer<'_> {
    pub fn continuation(mut self) -> Self {
        self.place = Place::Middle;
        self
    }
}

impl fmt::Write for Writer<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        use None as New;
        use Some as Line;

        for line in s.split('\n').map(Line).intersperse(New) {
            match self.place {
                Place::Start
                    if let Line(line) = line
                        && !line.is_empty() =>
                {
                    let wrote = (self.with)(self.writer)?;

                    if wrote > self.count {
                        panic!(
                            "indent writer wrote ({wrote}) more than the indent ({count})",
                            count = self.count
                        );
                    }

                    write!(self.writer, "{:>count$}", "", count = self.count - wrote)?;
                    self.place = Place::Middle;
                },

                Place::End => {
                    writeln!(self.writer)?;
                    self.place = Place::Start;
                },

                _ => {},
            }

            match line {
                New => self.place = Place::End,

                Line(line) => {
                    write!(self.writer, "{line}")?;
                },
            }
        }

        Ok(())
    }
}

#[doc(hidden)]
#[macro_export]
macro_rules! __indent {
    ($writer:ident,header = $header:expr) => {
        let header_width = {
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
            ::unicode_width::UnicodeWidthStr::width(header.to_str()) + 1
        };

        let mut wrote = false;
        $crate::indent::indent!(
            $writer,
            header_width,
            with = move |writer: &mut dyn fmt::Write| {
                if wrote {
                    return Ok(0);
                }

                write!(writer, "{header} ", header = $header)?;

                wrote = true;
                Ok(header_width)
            }
        );
    };

    ($writer:ident, $count:expr,continue: true) => {
        let $writer = &mut $crate::indent::indent($writer, $count).continuation();
    };

    ($writer:ident, $count:expr $(,continue: false)?) => {
        let $writer = &mut $crate::indent::indent($writer, $count);
    };

    ($writer:ident, $count:expr,continue: true,with = $with:expr) => {
        let mut with = $with;
        let $writer = &mut $crate::indent::indent_with($writer, $count, &mut with).continuation();
    };

    ($writer:ident, $count:expr $(,continue: false)?,with = $with:expr) => {
        let mut with = $with;
        let $writer = &mut $crate::indent::indent_with($writer, $count, &mut with);
    };
}

pub fn indent(writer: &mut dyn fmt::Write, count: usize) -> Writer<'_> {
    static mut ZERO_INDENTER: IndentWith<'static> = &mut |_| Ok(0);

    Writer {
        writer,
        // SAFETY: ZERO_INDENTER does not modify anything and the pointee of self.writer in Writer
        // is never replaced. Therefore we can use it, because without writes you can't have
        // race conditions.
        with: unsafe { ZERO_INDENTER },
        count,
        place: Place::Start,
    }
}

pub fn indent_with<'a>(writer: &'a mut dyn fmt::Write, count: usize, with: IndentWith<'a>) -> Writer<'a> {
    Writer {
        writer,
        with,
        count,
        place: Place::Start,
    }
}

#[doc(inline)]
pub use crate::__indent as indent;

#[doc(hidden)]
#[macro_export]
macro_rules! __dedent {
    ($writer:ident) => {
        let $writer = &mut Writer {
            writer: $writer.writer,
            count: 0,
            with: &mut move |_| Ok(0),
            place: $writer.place,
        };
    };

    ($writer:ident, $count:expr) => {
        let $writer = &mut Writer {
            writer: $writer.writer,
            count: $writer.count.checked_sub($count).expect("dedented too hard"),
            with: $writer.with,
            place: $writer.place,
        };
    };
}

#[doc(inline)]
pub use crate::__dedent as dedent;
