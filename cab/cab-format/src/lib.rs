//! Text formatting.

#![feature(iter_intersperse, if_let_guard, let_chains)]

use std::{
   fmt,
   io,
};

mod color;

pub use color::COLORS;

mod indent;
pub use indent::{
   IndentWith,
   IndentWriter,
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

pub trait WriteView: fmt::Write {
   fn width(&self) -> usize;

   fn width_max(&self) -> usize;
}

#[derive(Debug, Clone)]
pub struct View<W: fmt::Write> {
   writer: W,

   width:     usize,
   width_max: usize,
}

impl<W: fmt::Write> fmt::Write for View<W> {
   fn write_str(&mut self, s: &str) -> fmt::Result {
      use None as Newline;
      use Some as Line;

      let mut segments = s.split('\n').map(Line).intersperse(Newline).peekable();
      while let Some(segment) = segments.next() {
         match segment {
            Line(line) => {
               self.writer.write_str(line)?;

               if segments.peek().is_none() {
                  self.width += width(line);
               }
            },

            Newline => self.width = 0,
         }
      }

      Ok(())
   }
}

impl<W: fmt::Write> WriteView for View<W> {
   fn width(&self) -> usize {
      self.width
   }

   fn width_max(&self) -> usize {
      self.width_max
   }
}

impl<W: fmt::Write> From<W> for View<W> {
   fn from(writer: W) -> Self {
      Self {
         writer,

         width: 0,
         width_max: usize::MAX,
      }
   }
}

pub struct WriteFmt<T>(T);

impl<W: io::Write> fmt::Write for WriteFmt<W> {
   fn write_str(&mut self, s: &str) -> fmt::Result {
      self.0.write_all(s.as_bytes()).map_err(|_| fmt::Error)
   }
}

#[must_use]
pub fn stdout() -> View<impl fmt::Write> {
   View::from(WriteFmt(io::stdout()))
}

#[must_use]
pub fn stderr() -> View<impl fmt::Write> {
   View::from(WriteFmt(io::stderr()))
}

pub trait DisplayView {
   fn fmt(&self, writer: &mut dyn WriteView) -> fmt::Result;

   fn view_width(&self, width: usize) -> impl fmt::Display + '_
   where
      Self: Sized,
   {
      struct DisplayTerminal<'a, D: DisplayView> {
         display: &'a D,
         width:   usize,
      }

      impl<D: DisplayView> fmt::Display for DisplayTerminal<'_, D> {
         fn fmt(&self, writer: &mut fmt::Formatter<'_>) -> fmt::Result {
            let mut viewed = View::from(writer);

            viewed.width = self.width;

            DisplayView::fmt(self.display, &mut viewed)
         }
      }

      DisplayTerminal {
         display: self,
         width,
      }
   }

   fn view_terminal(&self) -> impl fmt::Display + '_
   where
      Self: Sized,
   {
      let width = if let Some((width, _)) = terminal_size::terminal_size() {
         width.0 as _
      } else {
         usize::MAX
      };

      self.view_width(width)
   }
}

impl<D: fmt::Display> DisplayView for D {
   fn fmt(&self, writer: &mut dyn WriteView) -> fmt::Result {
      write!(writer, "{self}")
   }
}

#[doc(hidden)]
pub mod private {
   pub use scopeguard::guard;

   pub use super::indent::IndentPlace;
}
