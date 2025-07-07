//! Runtime implementation.

#![feature(iter_intersperse, str_from_raw_parts)]

use cab_span::Span;

mod code;
pub use code::{
   ByteIndex,
   Code,
   ValueIndex,
};

mod compiler;
pub use compiler::{
   Compile,
   CompileOracle,
};

mod operation;
pub use operation::{
   Argument,
   Operation,
};

pub mod value;
pub use value::Value;

pub type Location = (value::Path, Span);
