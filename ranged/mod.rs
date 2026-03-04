//! Span and related type definitions.

#![warn(missing_docs)]

mod size;
pub use size::{
   IntoSize,
   Size,
};

mod span;
pub use span::{
   IntoSpan,
   Span,
   Spanned,
   SpannedExt,
};
