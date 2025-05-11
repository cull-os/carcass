use std::{
   fmt,
   io,
};

mod style;
pub use style::{
   Attr,
   Color,
   Style,
   Styled,
   StyledExt,
};

pub trait DisplayStyled<W: WriteStyled> {
   fn display_styled(&self, w: &mut W) -> io::Result<()>;
}

pub trait WriteStyled: io::Write {
   fn write_styled<T: fmt::Display>(&mut self, value: &Styled<T>) -> io::Result<()>;

   fn finish(&mut self) -> io::Result<()>;
}

pub fn terminal(inner: impl io::Write) -> impl WriteStyled {
   const RESET: &[u8] = b"\x1B[0m";

   #[derive(Debug, Clone, Copy, PartialEq, Eq)]
   enum Variant {
      Fg,
      Bg,
   }

   fn fg(color: Color) -> &'static [u8] {
      match color {
         Color::Primary => b"39",
         Color::Fixed(_) | Color::Rgb(..) => b"38",
         Color::Black => b"30",
         Color::Red => b"31",
         Color::Green => b"32",
         Color::Yellow => b"33",
         Color::Blue => b"34",
         Color::Magenta => b"35",
         Color::Cyan => b"36",
         Color::White => b"37",
         Color::BrightBlack => b"90",
         Color::BrightRed => b"91",
         Color::BrightGreen => b"92",
         Color::BrightYellow => b"93",
         Color::BrightBlue => b"94",
         Color::BrightMagenta => b"95",
         Color::BrightCyan => b"96",
         Color::BrightWhite => b"97",
      }
   }

   fn bg(color: Color) -> &'static [u8] {
      match color {
         Color::Primary => b"49",
         Color::Fixed(_) | Color::Rgb(..) => b"48",
         Color::Black => b"40",
         Color::Red => b"41",
         Color::Green => b"42",
         Color::Yellow => b"43",
         Color::Blue => b"44",
         Color::Magenta => b"45",
         Color::Cyan => b"46",
         Color::White => b"47",
         Color::BrightBlack => b"100",
         Color::BrightRed => b"101",
         Color::BrightGreen => b"102",
         Color::BrightYellow => b"103",
         Color::BrightBlue => b"104",
         Color::BrightMagenta => b"105",
         Color::BrightCyan => b"106",
         Color::BrightWhite => b"107",
      }
   }

   fn attr(attr: Attr) -> &'static [u8] {
      match attr {
         Attr::Bold => b"1",
         Attr::Dim => b"2",
         Attr::Italic => b"3",
         Attr::Underline => b"4",
         Attr::Blink => b"5",
         Attr::RapidBlink => b"6",
         Attr::Invert => b"7",
         Attr::Conceal => b"8",
         Attr::Strike => b"9",
      }
   }

   fn unattr(attr: Attr) -> &'static [u8] {
      match attr {
         Attr::Bold => b"22",
         Attr::Dim => b"22",
         Attr::Italic => b"23",
         Attr::Underline => b"24",
         Attr::Blink => b"25",
         Attr::RapidBlink => b"25",
         Attr::Invert => b"27",
         Attr::Conceal => b"28",
         Attr::Strike => b"29",
      }
   }

   fn write_color_start(
      writer: &mut impl io::Write,
      color: Color,
      variant: Variant,
   ) -> io::Result<()> {
      writer.write_all(match variant {
         Variant::Fg => fg(color),
         Variant::Bg => bg(color),
      })?;

      match color {
         Color::Fixed(num) => {
            let mut buffer = itoa::Buffer::new();

            writer.write_all(b";5;")?;
            writer.write_all(buffer.format(num).as_bytes())
         },

         Color::Rgb(r, g, b) => {
            let mut buffer = itoa::Buffer::new();

            writer.write_all(b";2;")?;
            writer.write_all(buffer.format(r).as_bytes())?;
            writer.write_all(b";")?;
            writer.write_all(buffer.format(g).as_bytes())?;
            writer.write_all(b";")?;
            writer.write_all(buffer.format(b).as_bytes())
         },

         _ => Ok(()),
      }
   }

   struct TerminalWriter<W: io::Write> {
      inner:   W,
      style:   Style,
      written: bool,
   }

   impl<W: io::Write> io::Write for TerminalWriter<W> {
      fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
         self.inner.write_all(RESET)?;
         self.style = Style::default();

         self.inner.write(buf)
      }

      fn flush(&mut self) -> io::Result<()> {
         self.inner.flush()
      }
   }

   impl<W: io::Write> WriteStyled for TerminalWriter<W> {
      fn write_styled<T: fmt::Display>(&mut self, styled: &Styled<T>) -> io::Result<()> {
         struct Splicer {
            written: bool,
         }

         impl Splicer {
            fn splice(&mut self, writer: &mut impl io::Write) -> io::Result<()> {
               if self.written {
                  writer.write_all(b";")
               } else {
                  self.written = true;
                  writer.write_all(b"\x1B[")
               }
            }

            fn finish(self, writer: &mut impl io::Write) -> io::Result<()> {
               if self.written {
                  writer.write_all(b"m")
               } else {
                  Ok(())
               }
            }
         }

         let Style {
            fg: fg_old,
            bg: bg_old,
            attrs: attrs_old,
         } = self.style;

         let Style {
            fg: fg_new,
            bg: bg_new,
            attrs: attrs_new,
         } = styled.style;

         let mut splicer = Splicer { written: false };

         if fg_old != fg_new {
            self.style.fg = fg_new;

            splicer.splice(&mut self.inner)?;
            write_color_start(&mut self.inner, fg_new, Variant::Fg)?;
         }

         if bg_old != bg_new {
            self.style.bg = bg_new;

            splicer.splice(&mut self.inner)?;
            write_color_start(&mut self.inner, bg_new, Variant::Bg)?;
         }

         for attr_deleted in attrs_old.difference(attrs_new) {
            splicer.splice(&mut self.inner)?;
            self.inner.write_all(unattr(attr_deleted))?;
         }

         for attr_added in attrs_new.difference(attrs_old) {
            splicer.splice(&mut self.inner)?;
            self.inner.write_all(attr(attr_added))?;
         }

         if splicer.written {
            self.written = true;
         }

         splicer.finish(&mut self.inner)?;

         write!(self.inner, "{value}", value = **styled)
      }

      fn finish(&mut self) -> io::Result<()> {
         if self.written {
            self.inner.write_all(RESET)
         } else {
            Ok(())
         }
      }
   }

   TerminalWriter {
      inner,
      style: Style::default(),
      written: false,
   }
}
