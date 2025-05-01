#[doc(inline)]
pub use yansi::{
   Attribute,
   Color,
   Condition,
   Painted as Styled,
   Quirk,
   Style,
};

pub const GUTTER: Style = Style::new().blue();
pub const HEADER_PATH: Style = Style::new().green();
pub const HEADER_POSITION: Style = Style::new().blue();

pub const RIGHT_TO_BOTTOM: char = '┏';
pub const TOP_TO_BOTTOM: char = '┃';
pub const TOP_TO_BOTTOM_PARTIAL: char = '┇';
pub const DOT: char = '·';
pub const TOP_TO_RIGHT: char = '┗';
pub const LEFT_TO_RIGHT: char = '━';
pub const LEFT_TO_TOP_BOTTOM: char = '┫';

pub const TOP_TO_BOTTOM_LEFT: char = '▏';
pub const TOP_LEFT_TO_RIGHT: char = '╲';
pub const TOP_TO_BOTTOM_RIGHT: char = '▕';

pub(crate) fn init() {
   yansi::whenever(yansi::Condition::TTY_AND_COLOR);
}

macro_rules! wrap {
   ($($prop:ident),* $(,)?) => {
      $(fn $prop(self) -> Styled<Self> {
         yansi::Painted::$prop(self.styled())
      })*
   };
}

pub trait StyleExt
where
   Self: Sized,
{
   fn styled(self) -> Styled<Self> {
      yansi::Paint::new(self)
   }

   fn style(self, style: impl Into<Style>) -> Styled<Self> {
      let mut colored = Styled::new(self);
      colored.style = style.into();
      colored
   }

   fn fixed(self, fixed: u8) -> Styled<Self> {
      yansi::Painted::fixed(self.styled(), fixed)
   }

   fn on_fixed(self, fixed: u8) -> Styled<Self> {
      yansi::Painted::on_fixed(self.styled(), fixed)
   }

   fn rgb(self, r: u8, g: u8, b: u8) -> Styled<Self> {
      yansi::Painted::rgb(self.styled(), r, g, b)
   }

   fn on_rgb(self, r: u8, g: u8, b: u8) -> Styled<Self> {
      yansi::Painted::on_rgb(self.styled(), r, g, b)
   }

   wrap![
      black,
      bright_black,
      on_black,
      on_bright_black,
      blue,
      bright_blue,
      on_blue,
      on_bright_blue,
      cyan,
      bright_cyan,
      on_cyan,
      on_bright_cyan,
      green,
      bright_green,
      on_green,
      on_bright_green,
      magenta,
      bright_magenta,
      on_magenta,
      on_bright_magenta,
      primary,
      on_primary,
      red,
      bright_red,
      on_red,
      on_bright_red,
      white,
      bright_white,
      on_white,
      on_bright_white,
      yellow,
      bright_yellow,
      on_yellow,
      on_bright_yellow,
      bold,
      dim,
      italic,
      underline,
      blink,
      rapid_blink,
      invert,
      conceal,
      strike,
      mask,
      wrap,
      linger,
      resetting,
      bright,
      on_bright,
   ];
}

impl<T> StyleExt for T {}
