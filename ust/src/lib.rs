use std::{
   fmt,
   io,
};

use derive_more::{
   Deref,
   DerefMut,
};
use enumset::{
   EnumSet,
   EnumSetType,
};

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Color {
   #[default]
   Primary,
   Fixed(u8),
   Rgb(u8, u8, u8),
   Black,
   Red,
   Green,
   Yellow,
   Blue,
   Magenta,
   Cyan,
   White,
   BrightBlack,
   BrightRed,
   BrightGreen,
   BrightYellow,
   BrightBlue,
   BrightMagenta,
   BrightCyan,
   BrightWhite,
}

#[derive(EnumSetType, Debug, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Attr {
   Bold,
   Dim,
   Italic,
   Underline,
   Blink,
   RapidBlink,
   Invert,
   Conceal,
   Strike,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Style {
   fg:    Color,
   bg:    Color,
   attrs: EnumSet<Attr>,
}

macro_rules! set {
   ($($name:ident : $field:ident $symbol:tt $value:expr;)*) => {
      $(
         #[must_use]
         pub fn $name(mut self) -> Style {
            self.$field $symbol $value;
            self
         }
      )*
   };
}

impl Style {
   #[must_use]
   pub fn new() -> Self {
      Self::default()
   }

   #[must_use]
   pub fn fg(mut self, color: Color) -> Self {
      self.fg = color;
      self
   }

   #[must_use]
   pub fn unfg(mut self) -> Self {
      self.fg = Color::default();
      self
   }

   #[must_use]
   pub fn bg(mut self, color: Color) -> Self {
      self.bg = color;
      self
   }

   #[must_use]
   pub fn unbg(mut self) -> Self {
      self.bg = Color::default();
      self
   }

   #[must_use]
   pub fn attr(mut self, attrs: impl Into<EnumSet<Attr>>) -> Self {
      self.attrs.insert_all(attrs.into());
      self
   }

   #[must_use]
   pub fn unattr(mut self, attrs: impl Into<EnumSet<Attr>>) -> Self {
      self.attrs.remove_all(attrs.into());
      self
   }

   #[must_use]
   pub fn fixed(mut self, color: u8) -> Self {
      self.fg = Color::Fixed(color);
      self
   }

   #[must_use]
   pub fn on_fixed(mut self, color: u8) -> Self {
      self.bg = Color::Fixed(color);
      self
   }

   #[must_use]
   pub fn rgb(mut self, r: u8, b: u8, g: u8) -> Self {
      self.fg = Color::Rgb(r, g, b);
      self
   }

   #[must_use]
   pub fn on_rgb(mut self, r: u8, b: u8, g: u8) -> Self {
      self.bg = Color::Rgb(r, g, b);
      self
   }

   set! {
      black:   fg = Color::Black;
      red:     fg = Color::Red;
      green:   fg = Color::Green;
      yellow:  fg = Color::Yellow;
      blue:    fg = Color::Blue;
      magenta: fg = Color::Magenta;
      cyan:    fg = Color::Cyan;
      white:   fg = Color::White;

      on_black:   bg = Color::Black;
      on_red:     bg = Color::Red;
      on_green:   bg = Color::Green;
      on_yellow:  bg = Color::Yellow;
      on_blue:    bg = Color::Blue;
      on_magenta: bg = Color::Magenta;
      on_cyan:    bg = Color::Cyan;
      on_white:   bg = Color::White;

      bright_black:   fg = Color::BrightBlack;
      bright_red:     fg = Color::BrightRed;
      bright_green:   fg = Color::BrightGreen;
      bright_yellow:  fg = Color::BrightYellow;
      bright_blue:    fg = Color::BrightBlue;
      bright_magenta: fg = Color::BrightMagenta;
      bright_cyan:    fg = Color::BrightCyan;
      bright_white:   fg = Color::BrightWhite;

      on_bright_black:   bg = Color::BrightBlack;
      on_bright_red:     bg = Color::BrightRed;
      on_bright_green:   bg = Color::BrightGreen;
      on_bright_yellow:  bg = Color::BrightYellow;
      on_bright_blue:    bg = Color::BrightBlue;
      on_bright_magenta: bg = Color::BrightMagenta;
      on_bright_cyan:    bg = Color::BrightCyan;
      on_bright_white:   bg = Color::BrightWhite;

      bold:        attrs |= Attr::Bold;
      dim:         attrs |= Attr::Dim;
      italic:      attrs |= Attr::Italic;
      underline:   attrs |= Attr::Underline;
      blink:       attrs |= Attr::Blink;
      rapid_blink: attrs |= Attr::RapidBlink;
      invert:      attrs |= Attr::Invert;
      conceal:     attrs |= Attr::Conceal;
      strike:      attrs |= Attr::Strike;
   }
}

#[derive(Deref, DerefMut, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Styled<T> {
   #[deref]
   #[deref_mut]
   value:     T,
   pub style: Style,
}

impl<T> Styled<T> {
   pub fn into_inner(self) -> T {
      self.value
   }
}

macro_rules! wrap_styled {
   ($($method:ident),* $(,)?) => {
      $(
         #[must_use]
         fn $method(self) -> Styled<Self> {
            let mut styled = self.styled();
            styled.style = styled.style.$method();
            styled
         }
      )*
   };
}

pub trait StyledExt
where
   Self: Sized,
{
   fn styled(self) -> Styled<Self> {
      Styled {
         value: self,
         style: Style::default(),
      }
   }

   #[must_use]
   fn fg(self, color: Color) -> Styled<Self> {
      let mut styled = self.styled();
      styled.style = styled.style.fg(color);
      styled
   }

   #[must_use]
   fn bg(self, color: Color) -> Styled<Self> {
      let mut styled = self.styled();
      styled.style = styled.style.bg(color);
      styled
   }

   #[must_use]
   fn attr(self, attrs: impl Into<EnumSet<Attr>>) -> Styled<Self> {
      let mut styled = self.styled();
      styled.style = styled.style.attr(attrs);
      styled
   }

   #[must_use]
   fn fixed(self, color: u8) -> Styled<Self> {
      let mut styled = self.styled();
      styled.style = styled.style.fixed(color);
      styled
   }

   #[must_use]
   fn on_fixed(self, color: u8) -> Styled<Self> {
      let mut styled = self.styled();
      styled.style = styled.style.on_fixed(color);
      styled
   }

   #[must_use]
   fn rgb(self, r: u8, b: u8, g: u8) -> Styled<Self> {
      let mut styled = self.styled();
      styled.style = styled.style.rgb(r, g, b);
      styled
   }

   #[must_use]
   fn on_rgb(self, r: u8, b: u8, g: u8) -> Styled<Self> {
      let mut styled = self.styled();
      styled.style = styled.style.on_rgb(r, g, b);
      styled
   }

   wrap_styled! {
      black,
      red,
      green,
      yellow,
      blue,
      magenta,
      cyan,
      white,

      on_black,
      on_red,
      on_green,
      on_yellow,
      on_blue,
      on_magenta,
      on_cyan,
      on_white,

      bright_black,
      bright_red,
      bright_green,
      bright_yellow,
      bright_blue,
      bright_magenta,
      bright_cyan,
      bright_white,

      on_bright_black,
      on_bright_red,
      on_bright_green,
      on_bright_yellow,
      on_bright_blue,
      on_bright_magenta,
      on_bright_cyan,
      on_bright_white,

      bold,
      dim,
      italic,
      underline,
      blink,
      rapid_blink,
      invert,
      conceal,
      strike,
   }
}

impl<T> StyledExt for T {}

pub trait DisplayStyled {
   fn display_styled(&self, w: &mut dyn WriteStyled) -> io::Result<()>;
}

pub trait WriteStyled: io::Write {
   fn write_styled(&mut self, value: Styled<&mut dyn fmt::Display>) -> io::Result<()>;

   fn finish(&mut self) -> io::Result<()>;

   fn terminal(inner: impl io::Write) -> impl WriteStyled
   where
      Self: Sized,
   {
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
         fn write_styled(&mut self, styled: Styled<&mut dyn fmt::Display>) -> io::Result<()> {
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

            write!(self.inner, "{value}", value = styled.value)
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
}
