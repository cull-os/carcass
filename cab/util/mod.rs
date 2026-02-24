//! Miscellaneous utilities.

mod lazy;
mod reffed;

pub mod suffix;

#[doc(hidden)]
pub mod private {
   pub use paste::paste;
}

/// Rebind an identifier with concise call-chain syntax.
///
/// # Example
///
/// ```rs
/// call!(mut foo.bar.baz());
/// // let mut foo = foo.bar.baz();
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
      #[doc = concat!("Alias for `call!` with `", stringify!($($call)+), "` as the suffix, with multiple identifier support.")]
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
