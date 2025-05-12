use derive_more::{
   Deref,
   DerefMut,
};
use enumflags2::{
   BitFlags,
   bitflags,
};

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
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

impl Color {
   pub const fn fg(self) -> Style {
      Style::new().fg(self)
   }

   pub const fn bg(self) -> Style {
      Style::new().bg(self)
   }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[bitflags]
#[repr(u16)]
pub enum Attr {
   Bold       = 1 << 0,
   Dim        = 1 << 1,
   Italic     = 1 << 2,
   Underline  = 1 << 3,
   Blink      = 1 << 4,
   RapidBlink = 1 << 5,
   Invert     = 1 << 6,
   Conceal    = 1 << 7,
   Strike     = 1 << 8,
}

impl Attr {
   pub const fn style(self) -> Style {
      Style::new().attr(self)
   }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Style {
   pub fg:    Color,
   pub bg:    Color,
   pub attrs: BitFlags<Attr>,
}

macro_rules! set {
   ($($name:ident : $field:ident $symbol:tt $value:expr;)*) => {
      $(
         #[must_use]
         pub const fn $name(mut self) -> Style {
            self.$field $symbol $value;
            self
         }
      )*
   };
}

macro_rules! set_attr {
   ($($name:ident : $attr:expr;)*) => {
      $(
         #[must_use]
         pub const fn $name(mut self) -> Style {
            self.attrs = self.attrs.union_c(BitFlags::<Attr>::from_bits_truncate_c(
               $attr as u16,
               BitFlags::CONST_TOKEN,
            ));
            self
         }
      )*
   };
}

impl Style {
   #[must_use]
   pub const fn new() -> Self {
      Self {
         fg:    Color::Primary,
         bg:    Color::Primary,
         attrs: BitFlags::EMPTY,
      }
   }

   #[must_use]
   pub const fn fg(mut self, color: Color) -> Self {
      self.fg = color;
      self
   }

   #[must_use]
   pub const fn unfg(mut self) -> Self {
      self.fg = Color::Primary;
      self
   }

   #[must_use]
   pub const fn bg(mut self, color: Color) -> Self {
      self.bg = color;
      self
   }

   #[must_use]
   pub const fn unbg(mut self) -> Self {
      self.bg = Color::Primary;
      self
   }

   #[must_use]
   pub const fn attr(mut self, attr: Attr) -> Self {
      self.attrs = self.attrs.union_c(BitFlags::<Attr>::from_bits_truncate_c(
         attr as u16,
         BitFlags::CONST_TOKEN,
      ));
      self
   }

   #[must_use]
   pub const fn unattr(mut self, attr: Attr) -> Self {
      self.attrs = self.attrs.intersection_c(
         BitFlags::<Attr>::from_bits_truncate_c(attr as u16, BitFlags::CONST_TOKEN)
            .not_c(BitFlags::CONST_TOKEN),
      );
      self
   }

   #[must_use]
   pub const fn fixed(mut self, color: u8) -> Self {
      self.fg = Color::Fixed(color);
      self
   }

   #[must_use]
   pub const fn on_fixed(mut self, color: u8) -> Self {
      self.bg = Color::Fixed(color);
      self
   }

   #[must_use]
   pub const fn rgb(mut self, r: u8, b: u8, g: u8) -> Self {
      self.fg = Color::Rgb(r, g, b);
      self
   }

   #[must_use]
   pub const fn on_rgb(mut self, r: u8, b: u8, g: u8) -> Self {
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
   }

   set_attr! {
      bold:        Attr::Bold;
      dim:         Attr::Dim;
      italic:      Attr::Italic;
      underline:   Attr::Underline;
      blink:       Attr::Blink;
      rapid_blink: Attr::RapidBlink;
      invert:      Attr::Invert;
      conceal:     Attr::Conceal;
      strike:      Attr::Strike;
   }
}

#[derive(Deref, DerefMut, Debug, Clone, PartialEq, Eq)]
pub struct Styled<T> {
   #[deref]
   #[deref_mut]
   pub value: T,
   pub style: Style,
}

macro_rules! styled {
   ($($method:ident),* $(,)?) => {
      $(
         #[must_use]
         pub const fn $method(mut self) -> Self {
            self.style = self.style.$method();
            self
         }
      )*
   };
}

impl<T> Styled<T> {
   #[must_use]
   pub const fn fg(mut self, color: Color) -> Self {
      self.style = self.style.fg(color);
      self
   }

   #[must_use]
   pub const fn bg(mut self, color: Color) -> Self {
      self.style = self.style.bg(color);
      self
   }

   #[must_use]
   pub const fn attr(mut self, attr: Attr) -> Self {
      self.style = self.style.attr(attr);
      self
   }

   #[must_use]
   pub const fn fixed(mut self, color: u8) -> Self {
      self.style = self.style.fixed(color);
      self
   }

   #[must_use]
   pub const fn on_fixed(mut self, color: u8) -> Self {
      self.style = self.style.on_fixed(color);
      self
   }

   #[must_use]
   pub const fn rgb(mut self, r: u8, b: u8, g: u8) -> Self {
      self.style = self.style.rgb(r, g, b);
      self
   }

   #[must_use]
   pub const fn on_rgb(mut self, r: u8, b: u8, g: u8) -> Self {
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
   fn style(self, style: Style) -> Styled<Self> {
      Styled { value: self, style }
   }

   fn styled(self) -> Styled<Self> {
      self.style(Style::default())
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
   fn attr(self, attr: Attr) -> Styled<Self> {
      let mut styled = self.styled();
      styled.style = styled.style.attr(attr);
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
