//! Miscellaneous utilities.

mod lazy;
mod reffed;

pub mod suffix;

#[doc(hidden)]
pub mod private {
   pub use paste::paste;
}

/// A macro to make mass redeclarations of a collection of identifiers using a
/// single method more concise.
///
/// # Example
///
/// ```rs
/// // This:
/// call!(foo; bar, qux);
///
/// // Gets turned into this:
/// let bar = bar.foo();
/// let qux = qux.foo();
/// ```
#[macro_export]
macro_rules! call {
   ($method:ident; $($identifier:ident),*) => {
      $(let $identifier = $identifier.$method();)*
   }
}

/// [`call!`], but the identifier is mutable.
#[macro_export]
macro_rules! call_mut {
   ($method:ident; $($identifier:ident),*) => {
      $(let mut $identifier = $identifier.$method();)*
   }
}

/// [`call!`] but with the method set to `as_`.
#[macro_export]
macro_rules! as_ {
   ($($t:tt),*) => {
      $crate::call!(as_; $($t),*);
   }
}

/// [`call!`] but with the method set to `as_ref`.
#[macro_export]
macro_rules! as_ref {
   ($($t:tt),*) => {
      $crate::call!(as_ref; $($t),*);
   }
}

/// [`call_mut!`] but with the method set to `borrow_mut`.
#[macro_export]
macro_rules! borrow_mut {
   ($($t:tt),*) => {
      $crate::call_mut!(borrow_mut; $($t),*);
   }
}

/// [`call!`] but with the method set to `clone`.
#[macro_export]
macro_rules! clone {
   ($($t:tt),*) => {
      $crate::call!(clone; $($t),*);
   }
}

/// [`call!`] but with the method set to `into`.
#[macro_export]
macro_rules! into {
   ($($t:tt),*) => {
      $crate::call!(into; $($t),*);
   }
}

/// [`call_mut!`] but with the method set to `into_iter`.
#[macro_export]
macro_rules! into_iter {
   ($($t:tt),*) => {
      $crate::call_mut!(into_iter; $($t),*);
   }
}

/// [`call!`] but with the method set to `unwrap`.
#[macro_export]
macro_rules! unwrap {
   ($($t:tt),*) => {
      $crate::call!(unwrap; $($t),*);
   }
}
