use std::borrow::Cow;

use yansi::Paint as _;

use crate::into;

/// A spanless label. Displayed at the end of the report.
#[derive(Debug, Clone)]
pub struct Point {
   /// The title of the label.
   pub title: yansi::Painted<Cow<'static, str>>,
   /// The text of the label.
   pub text: Cow<'static, str>,
}

impl Point {
   /// Creates a new [`Point`].
   pub fn new(
      title: yansi::Painted<impl Into<Cow<'static, str>>>,
      text: impl Into<Cow<'static, str>>,
   ) -> Self {
      let mut title2 = title.value.into().new();
      title2.style = title.style;

      into!(text);

      Self {
         title: title2,
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
