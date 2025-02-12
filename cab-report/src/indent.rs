use std::{
    cell::Cell,
    fmt,
    sync::LazyLock,
};

use unicode_width::UnicodeWidthStr;
use yansi::Paint as _;

static LINE_WIDTH_MAX: LazyLock<usize> = LazyLock::new(|| {
    terminal_size::terminal_size()
        .map(|(width, _)| width.0 as usize)
        .unwrap_or(120)
});

thread_local! {
    #[doc(hidden)]
    pub static LINE_WIDTH: Cell<usize> = const { Cell::new(0) };
}

pub fn writeln_wrapped<'a>(
    writer: &'a mut dyn fmt::Write,
    parts: impl Iterator<Item = yansi::Painted<&'a str>>,
) -> fmt::Result {
    use None as Space;
    use Some as Word;

    let mut line_width_max = *LINE_WIDTH_MAX;

    let line_width_start = LINE_WIDTH.get();
    let mut line_width = line_width_start;

    // If we can't even write any text just assume the line is uncapped.
    if line_width_start >= line_width_max {
        line_width_max = usize::MAX;
    }

    let mut parts = parts
        .flat_map(|part| {
            part.value
                .split(' ')
                .map(move |word| Word(word.paint(part.style)))
                .intersperse(Space)
        })
        .peekable();

    while let Some(part) = parts.peek_mut() {
        let Word(word) = part.as_mut() else {
            if line_width != 0 && line_width < line_width_max {
                write!(writer, " ")?;
                line_width += 1;
            }

            parts.next();
            continue;
        };

        let word_width = word.value.width();

        // Word fits in current line.
        if line_width + word_width < line_width_max {
            write!(writer, "{word}")?;
            line_width += word_width;

            parts.next();
            continue;
        }

        // Word fits in the next line.
        if line_width_start + word_width < line_width_max {
            writeln!(writer)?;
            line_width = line_width_start;

            write!(writer, "{word}")?;
            line_width += word_width;

            parts.next();
            continue;
        }

        // Word doesn't fit in the next line.
        let line_width_remainder = line_width_max - line_width;

        let split_index = word
            .value
            .char_indices()
            .enumerate()
            .find_map(|(index, (split_index, _))| (index + 1 >= line_width_remainder).then_some(split_index))
            .unwrap();

        let (word_this, word_rest) = word.value.split_at(split_index);

        word.value = word_this;
        writeln!(writer, "{word}")?;
        line_width = line_width_start;

        word.value = word_rest;
    }

    writeln!(writer)
}

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
    pub fn write_indent(&mut self) -> fmt::Result {
        assert_eq!(self.place, Place::Start);

        let wrote = (self.with)(self.writer)?;

        if wrote > self.count {
            panic!(
                "indent writer wrote ({wrote}) more than the indent ({count})",
                count = self.count
            );
        }

        write!(self.writer, "{:>count$}", "", count = self.count - wrote)?;
        self.place = Place::Middle;

        Ok(())
    }
}

impl Drop for Writer<'_> {
    fn drop(&mut self) {
        LINE_WIDTH.set(LINE_WIDTH.get().saturating_sub(self.count));
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
                    self.write_indent()?;
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
            ::unicode_width::UnicodeWidthStr::width(header.to_str())
        };

        let mut wrote = false;
        $crate::indent::indent!(
            $writer,
            header_width + 1,
            with = move |writer: &mut dyn fmt::Write| {
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
        let $writer = &mut $crate::indent::indent($writer, $count);
    };

    ($writer:ident, $count:expr,with = $with:expr) => {
        let mut with = $with;
        let $writer = &mut $crate::indent::indent_with($writer, $count, &mut with);
    };
}

pub fn indent(writer: &mut dyn fmt::Write, count: usize) -> Writer<'_> {
    static mut ZERO_INDENTER: IndentWith<'static> = &mut |_| Ok(0);

    LINE_WIDTH.set(LINE_WIDTH.get() + count);

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
    LINE_WIDTH.set(LINE_WIDTH.get() + count);

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
        $crate::indent::dedent!($writer, $writer.count, discard = true);
    };

    ($writer:ident, $count:expr) => {
        $crate::indent::dedent!($writer, $count, discard = true);
    };

    ($writer:ident, $count:expr,discard = $discard:literal) => {
        let count = $count;
        let old_count = $crate::indent::LINE_WIDTH.get();

        $crate::indent::LINE_WIDTH.set(old_count.saturating_sub(count));
        let _guard = ::scopeguard::guard((), |_| {
            $crate::indent::LINE_WIDTH.set(old_count);
        });

        let $writer = &mut $crate::indent::Writer {
            writer: $writer.writer,
            count: $writer.count.checked_sub(count).expect("dedented too hard"),
            with: if $discard { &mut move |_| Ok(0) } else { $writer.with },
            place: $writer.place,
        };
    };
}

#[doc(inline)]
pub use crate::__dedent as dedent;
