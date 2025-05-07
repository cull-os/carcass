use std::{
   fmt,
   io,
   os::fd::AsFd,
};

use itertools::Itertools as _;
use paste::paste;

use crate::width;

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
                  self.width = self.width.saturating_add(width(line));
               }
            },

            Newline => {
               self.writer.write_char('\n')?;
               self.width = 0;
            },
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

struct WriteFmt<T>(T);

impl<W: io::Write> fmt::Write for WriteFmt<W> {
   fn write_str(&mut self, s: &str) -> fmt::Result {
      self.0.write_all(s.as_bytes()).map_err(|_| fmt::Error)
   }
}

/// Constructs a new view to the given file descriptor.
pub fn fd(fd: impl AsFd + io::Write) -> View<impl fmt::Write> {
   let view_size = terminal_size::terminal_size_of(&fd);
   let mut view = View::from(WriteFmt(fd));

   if let Some((width, _)) = view_size {
      view.width_max = width.0 as _;
   }

   view
}

/// Constructs a new view to the standard output of the current process.
#[must_use]
pub fn stdout() -> View<impl fmt::Write> {
   fd(io::stdout())
}

/// Constructs a new view to the standard error of the current process.
#[must_use]
pub fn stderr() -> View<impl fmt::Write> {
   fd(io::stderr())
}

macro_rules! impl_view {
   ($type:ident, $ident:ident) => {
      paste! {
         pub trait [<$type View>] {
            fn $ident(&self, writer: &mut dyn WriteView) -> fmt::Result;

            fn [<$ident _width>](&self, width: usize) -> impl fmt::$type + '_
            where
               Self: Sized,
            {
               struct Terminal<'a, D: [<$type View>]> {
                  $ident: &'a D,
                  width:   usize,
               }

               impl<D: [<$type View>]> fmt::$type for Terminal<'_, D> {
                  fn fmt(&self, writer: &mut fmt::Formatter<'_>) -> fmt::Result {
                     let mut viewed = View::from(writer);

                     viewed.width = self.width;

                     [<$type View>]::$ident(self.$ident, &mut viewed)
                  }
               }

               Terminal {
                  $ident: self,
                  width,
               }
            }

            fn [<$ident _terminal_width>](&self) -> impl fmt::$type + '_
            where
               Self: Sized,
            {
               if let Some((width, _)) = terminal_size::terminal_size() {
                  self.[<$ident _width>](width.0 as _)
               } else {
                  self.[<$ident _width>](usize::MAX)
               }
            }

            fn [<$ident _free_width>](&self) -> impl fmt::$type + '_
            where
               Self: Sized,
            {
               self.[<$ident _width>](usize::MAX)
            }
         }
      }
   };
}

impl_view!(Display, display);

impl<D: fmt::Display> DisplayView for D {
   fn display(&self, writer: &mut dyn WriteView) -> fmt::Result {
      write!(writer, "{self}")
   }
}

impl_view!(Debug, debug);

impl<D: fmt::Debug> DebugView for D {
   fn debug(&self, writer: &mut dyn WriteView) -> fmt::Result {
      write!(writer, "{self:?}")
   }
}
