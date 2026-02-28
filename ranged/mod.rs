//! Span and related type definitions.

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
};
