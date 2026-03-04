//! Miscellneous utilities.

#![warn(missing_docs)]

mod lazy;
mod reffed;

pub mod suffix;

/// Internal re-exports used by crate macros.
#[doc(hidden)]
pub mod private {
   pub use paste::paste;
}

/// Rebinds an identifier to a transformed value using method-call syntax.
///
/// This macro lets you write `let <name> = <name>.<chain>();` in a compact
/// form that starts from the identifier. It is useful when repeatedly
/// rebinding a value through a short transformation pipeline.
///
/// # Example
///
/// ```rs
/// # use cab_util::call;
/// let values = vec![1_u8, 2, 3];
/// call!(values.into_iter().map(u16::from).collect::<Vec<_>>());
/// assert_eq!(values, vec![1_u16, 2, 3]);
///
/// let words = vec!["cab", "util"];
/// call!(mut words.into_iter().map(str::to_uppercase).collect::<Vec<_>>());
/// words.push(String::from("DONE"));
/// assert_eq!(words.last(), Some(&String::from("DONE")));
/// ```
#[macro_export]
macro_rules! call {
   (mut $identifier:ident $($call:tt)+ $(,)?) => {
      let mut $identifier = $identifier $($call)+;
   };

   ($identifier:ident $($call:tt)+ $(,)?) => {
      let $identifier = $identifier $($call)+;
   };
}

macro_rules! call_alias {
   ([$d:tt] $name:ident => $($call:tt)+) => {
      #[doc = concat!(
         "Alias for [`call!`] that appends `",
         stringify!($($call)+),
         "`. Supports comma-separated identifiers and optional `mut`."
      )]
      #[macro_export]
      macro_rules! $name {
         ($d(mut $d identifier:ident),* $d(,)?) => {
            $d(let mut $d identifier = $d identifier $($call)+;)*
         };

         ($d($d identifier:ident),* $d(,)?) => {
            $d(let $d identifier = $d identifier $($call)+;)*
         };
      }
   };

   ($($call:tt)+) => {
      call_alias!([$] $($call)+);
   };
}

call_alias!(collect_vec => .into_iter().collect::<Vec<_>>());
call_alias!(as_ => .as_());
call_alias!(as_ref => .as_ref());
call_alias!(borrow_mut => .borrow_mut());
call_alias!(clone => .clone());
call_alias!(into => .into());
call_alias!(into_iter => .into_iter());
call_alias!(unwrap => .unwrap());
