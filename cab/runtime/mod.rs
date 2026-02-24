//! Runtime implementation.

#![feature(
   gen_blocks,
   if_let_guard,
   impl_trait_in_assoc_type,
   iter_intersperse,
   str_from_raw_parts
)]

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

mod scope;
pub use scope::{
   Scope,
   ScopeId,
   Scopes,
};

mod state;
pub use state::State;

mod operation;
pub use operation::{
   Argument,
   Operation,
};

pub mod value;
pub use value::Value;
