use std::{
   fmt,
   sync::atomic,
};

use unicode_segmentation::UnicodeSegmentation as _;
use unicode_width::UnicodeWidthStr as _;
use yansi::Paint as _;

use crate::{
   call,
   text::{
      LINE_WIDTH,
      LINE_WIDTH_MAX,
   },
};

/// [`wrap`], but with a newline after the text.
pub fn wrapln<'a>(
   writer: &mut dyn fmt::Write,
   parts: impl IntoIterator<Item = yansi::Painted<&'a str>>,
) -> fmt::Result {
   wrap(writer, parts)?;
   writeln!(writer)
}

/// Writes the given iterator of colored words into the writer, splicing and
/// wrapping at the max line width.
pub fn wrap<'a>(
   writer: &mut dyn fmt::Write,
   parts: impl IntoIterator<Item = yansi::Painted<&'a str>>,
) -> fmt::Result {
   use None as Space;
   use Some as Word;

   call!(into_iter; parts);

   let line_width_start = LINE_WIDTH.load(atomic::Ordering::Acquire);
   let mut line_width = line_width_start;

   let line_width_max = if line_width_start < *LINE_WIDTH_MAX {
      *LINE_WIDTH_MAX
   } else {
      // If we can't even write any text just assume the line is uncapped.
      u16::MAX
   };

   let mut parts = parts
      .flat_map(|part| {
         part
            .value
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

      let word_width = word.value.width() as u16;

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
         .grapheme_indices(true)
         .enumerate()
         .find_map(|(index, (split_index, _))| {
            (index as u16 + 1 >= line_width_remainder).then_some(split_index)
         })
         .unwrap();

      let (word_this, word_rest) = word.value.split_at(split_index);

      word.value = word_this;
      writeln!(writer, "{word}")?;
      line_width = line_width_start;

      word.value = word_rest;
   }

   Ok(())
}
