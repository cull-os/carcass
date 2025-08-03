use std::ops;

use dup::Dupe;

#[derive(Clone, Dupe, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[expect(clippy::module_name_repetitions)]
pub struct SString(#[doc(hidden)] pub arcstr::Substr);

#[doc(hidden)]
pub mod private {
   pub use arcstr::literal_substr;
}

#[macro_export]
#[expect(clippy::module_name_repetitions)]
macro_rules! __string_new {
   ($s:literal $(,)?) => {
      $crate::value::SString($crate::value::string::private::literal_substr!($s))
   };
}

pub use crate::__string_new as new;

impl From<&str> for SString {
   fn from(s: &str) -> Self {
      Self(arcstr::Substr::from(s))
   }
}

impl ops::Deref for SString {
   type Target = str;

   fn deref(&self) -> &Self::Target {
      &self.0
   }
}
