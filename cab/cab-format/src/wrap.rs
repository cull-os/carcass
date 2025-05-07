use std::{
   fmt,
   num::NonZeroUsize,
};

use cab_util::into_iter;
use itertools::Itertools as _;
use unicode_segmentation::UnicodeSegmentation as _;

use crate::{
   WriteView,
   style::{
      StyleExt as _,
      Styled,
   },
   width,
};

const WIDTH_NEEDED: NonZeroUsize = NonZeroUsize::new(8).unwrap();

/// [`wrap`], but with a newline before the text.
pub fn lnwrap<'a>(
   writer: &mut dyn WriteView,
   parts: impl IntoIterator<Item = Styled<&'a str>>,
) -> fmt::Result {
   writeln!(writer)?;
   wrap(writer, parts)
}

/// Writes the given iterator of colored words into the writer, splicing and
/// wrapping at the max width.
pub fn wrap<'a>(
   writer: &mut dyn WriteView,
   parts: impl IntoIterator<Item = Styled<&'a str>>,
) -> fmt::Result {
   use None as Newline;
   use Some as Word;

   let mut parts = parts
      .into_iter()
      .flat_map(|part| {
         part
            .value
            .split('\n')
            .map(move |word| Word(word.style(part.style)))
            .intersperse(Newline)
      })
      .peekable();

   while parts.peek().is_some() {
      wrap_line(
         writer,
         parts
            .by_ref()
            .take_while_inclusive(|&part| matches!(part, Word(_)))
            .map(|part| {
               match part {
                  Word(word) => word,
                  Newline => "\n".styled(),
               }
            }),
      )?;
   }

   Ok(())
}

fn wrap_line<'a>(
   writer: &mut dyn WriteView,
   parts: impl IntoIterator<Item = Styled<&'a str>>,
) -> fmt::Result {
   use None as Space;
   use Some as Word;

   into_iter!(parts);

   let width_start = writer.width();

   let width_max = if width_start + WIDTH_NEEDED.get() <= writer.width_max() {
      writer.width_max()
   } else {
      // If we can't even write WIDTH_NEEDED amount just assume the width is
      // double the worst case width.
      (writer.width_max() + WIDTH_NEEDED.get()) * 2
   };

   let mut parts = parts
      .flat_map(|part| {
         part
            .value
            .split(' ')
            .map(move |word| Word(word.style(part.style)))
            .intersperse(Space)
      })
      .peekable();

   while let Some(part) = parts.peek_mut() {
      let Word(word) = part.as_mut() else {
         if writer.width() != 0 && writer.width() < width_max {
            write!(writer, " ")?;
         }

         parts.next();
         continue;
      };

      let word_width = width(word.value);

      // Word fits in current line.
      if writer.width() + word_width <= width_max {
         write!(writer, "{word}")?;

         parts.next();
         continue;
      }

      // Word fits in the next line.
      if width_start + word_width <= width_max {
         writeln!(writer)?;
         write!(writer, "{word}")?;

         parts.next();
         continue;
      }

      // Word doesn't fit in the next line.
      let width_remainder = width_max - writer.width();

      let split_index = word
         .value
         .grapheme_indices(true)
         .scan(0, |width, state @ (_, grapheme)| {
            *width += self::width(grapheme);
            Some((*width, state))
         })
         .find_map(|(width, (split_index, _))| (width > width_remainder).then_some(split_index))
         .unwrap();

      let (word_this, word_rest) = word.value.split_at(split_index);

      word.value = word_this;
      writeln!(writer, "{word}")?;

      word.value = word_rest;
   }

   Ok(())
}
