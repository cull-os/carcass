//! Text formatting.

#![feature(iter_intersperse, if_let_guard, let_chains)]

mod color;
use std::fmt;

pub use color::COLORS;

mod indent;
pub use indent::{
   IndentWith,
   IndentWriter,
   indent,
   indent_with,
};

pub mod style;

#[path = "width.rs"] mod width_;
pub use width_::{
   number_hex_width,
   number_width,
   width,
};

mod wrap;
pub use wrap::{
   lnwrap,
   wrap,
};

/// Initialize data required to format text.
pub fn init() {
   style::init();
}

pub trait Write {
   fn width(&self) -> usize;
   fn width_set(&mut self, width: usize);

   fn width_max(&self) -> usize;

   fn write_width(&mut self, s: &str) -> fmt::Result;
}

impl fmt::Write for dyn Write + '_ {
   fn write_str(&mut self, s: &str) -> fmt::Result {
      self.write_width(s)
   }
}

pub trait Display {
   fn fmt(&self, writer: &mut dyn Write) -> fmt::Result;
}

impl fmt::Display for dyn Display {
   fn fmt(&self, writer: &mut fmt::Formatter<'_>) -> fmt::Result {
      Display::fmt(self, &mut WriteImpl::from(writer as &mut dyn fmt::Write))
   }
}

struct WriteImpl<'a> {
   writer: &'a mut dyn fmt::Write,

   width:     usize,
   width_max: usize,
}

impl Write for WriteImpl<'_> {
   fn width(&self) -> usize {
      self.width
   }

   fn width_max(&self) -> usize {
      self.width_max
   }

   fn width_set(&mut self, width: usize) {
      self.width = width;
   }

   fn write_width(&mut self, s: &str) -> fmt::Result {
      use None as Newline;
      use Some as Line;

      let mut segments = s.split('\n').map(Line).intersperse(Newline).peekable();
      while let Some(segment) = segments.next() {
         match segment {
            Line(line) => {
               self.writer.write_str(line)?;

               if segments.peek().is_none() {
                  self.width_set(self.width() + width(line));
               }
            },

            Newline => self.width_set(0),
         }
      }

      Ok(())
   }
}

impl<'a> From<&'a mut dyn fmt::Write> for WriteImpl<'a> {
   fn from(writer: &'a mut dyn fmt::Write) -> Self {
      Self {
         writer,
         width: 0,
         width_max: usize::MAX,
      }
   }
}

impl WriteImpl<'_> {
   #[must_use]
   fn width_max_viewport(mut self) -> Self {
      let Some((width, _)) = terminal_size::terminal_size() else {
         return self;
      };

      self.width_max = width.0 as _;
      self
   }
}

#[doc(hidden)]
pub mod private {
   pub use scopeguard::guard;

   pub use super::indent::IndentPlace;
}
