//! Text and report formatting utilities and data types.

#![feature(
   gen_blocks,
   if_let_guard,
   iter_intersperse,
   let_chains,
   trait_alias,
   try_trait_v2
)]

#[cfg(feature = "error")]
mod error;
mod print;
mod report;
mod text;

#[cfg(feature = "error")]
pub use self::error::{
   Context,
   Contextful,
   Error,
   Result,
   Termination,
};
pub use self::{
   print::{
      IndentWith,
      IndentWriter,
      indent,
      indent_with,
      wrap,
      wrapln,
   },
   report::{
      Label,
      LabelSeverity,
      Point,
      Position,
      Report,
      ReportSeverity,
   },
   text::{
      IntoSize,
      IntoSpan,
      Size,
      Span,
   },
};

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
   ($method:ident; $($variable:ident),*) => {
      $(let $variable = $variable.$method();)*
   }
}

/// [`call!`] but with the method set to `as_ref`.
#[macro_export]
macro_rules! as_ref {
   ($($t:tt),*) => {
      $crate::call!(as_ref; $($t),*);
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

#[doc(hidden)]
pub mod __private {
   pub use anyhow;
   pub use scopeguard;
   pub use unicode_width;
   pub use yansi;

   pub use super::{
      print::IndentPlace,
      text::{
         LINE_WIDTH,
         LINE_WIDTH_MAX,
      },
   };
}
