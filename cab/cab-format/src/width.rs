use cab_util::as_;
use num::traits::AsPrimitive;
use unicode_segmentation::UnicodeSegmentation as _;

#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn number_width(number: impl AsPrimitive<f64>) -> usize {
   as_!(number);

   if number == 0.0 {
      1
   } else {
      number.log10() as usize + 1
   }
}

#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn number_hex_width(number: impl AsPrimitive<f64>) -> usize {
   as_!(number);

   if number == 0.0 {
      1
   } else {
      number.log(16.0) as usize + 1
   }
}

pub fn is_emoji(s: &str) -> bool {
   !s.is_ascii() && s.chars().any(unic_emoji_char::is_emoji)
}

#[must_use]
pub fn width(s: &str) -> usize {
   s.graphemes(true)
      .map(|grapheme| {
         match grapheme {
            "\t" => 4,
            s if is_emoji(s) => 2,
            #[expect(clippy::disallowed_methods)]
            s => unicode_width::UnicodeWidthStr::width(s),
         }
      })
      .sum::<usize>()
}
