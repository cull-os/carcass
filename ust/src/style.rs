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
   pub fg:    Color,
   pub bg:    Color,
   pub attrs: EnumSet<Attr>,
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

macro_rules! styled {
   ($($method:ident),* $(,)?) => {
      $(
         #[must_use]
         pub fn $method(mut self) -> Self {
            self.style = self.style.$method();
            self
         }
      )*
   };
}

impl<T> Styled<T> {
   pub fn into_inner(self) -> T {
      self.value
   }

   #[must_use]
   pub fn fg(mut self, color: Color) -> Self {
      self.style = self.style.fg(color);
      self
   }

   #[must_use]
   pub fn bg(mut self, color: Color) -> Self {
      self.style = self.style.bg(color);
      self
   }

   #[must_use]
   pub fn attr(mut self, attrs: impl Into<EnumSet<Attr>>) -> Self {
      self.style = self.style.attr(attrs);
      self
   }

   #[must_use]
   pub fn fixed(mut self, color: u8) -> Self {
      self.style = self.style.fixed(color);
      self
   }

   #[must_use]
   pub fn on_fixed(mut self, color: u8) -> Self {
      self.style = self.style.on_fixed(color);
      self
   }

   #[must_use]
   pub fn rgb(mut self, r: u8, b: u8, g: u8) -> Self {
      self.style = self.style.rgb(r, g, b);
      self
   }

   #[must_use]
   pub fn on_rgb(mut self, r: u8, b: u8, g: u8) -> Self {
      self.style = self.style.on_rgb(r, g, b);
      self
   }

   styled! {
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
