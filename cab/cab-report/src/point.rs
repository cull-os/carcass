use std::borrow::Cow;

use cab_format::style::{
   StyleExt as _,
   Styled,
};
use cab_util::into;

/// A spanless label. Displayed at the end of the report.
#[derive(Debug, Clone)]
pub struct Point {
   /// The title of the label.
   pub title: Styled<Cow<'static, str>>,
   /// The text of the label.
   pub text:  Cow<'static, str>,
}

impl Point {
   /// Creates a new [`Point`].
   pub fn new(
      title: Styled<impl Into<Cow<'static, str>>>,
      text: impl Into<Cow<'static, str>>,
   ) -> Self {
      into!(text);

      Self {
         title: title.value.into().style(title.style),
         text,
      }
   }

   /// Creates a tip [`Point`].
   pub fn tip(text: impl Into<Cow<'static, str>>) -> Self {
      Self::new("tip:".magenta().bold(), text)
   }

   /// Creates a help [`Point`].
   pub fn help(text: impl Into<Cow<'static, str>>) -> Self {
      Self::new("help:".cyan().bold(), text)
   }
}
