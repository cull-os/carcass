use derive_more::Deref;
use dup::Dupe;

#[derive(Deref, Clone, Dupe, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[expect(clippy::module_name_repetitions)]
pub struct SString(#[doc(hidden)] pub arcstr::Substr);

#[doc(hidden)]
pub mod private {
   pub use arcstr::literal_substr;
}

#[macro_export]
#[doc(hidden)]
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
