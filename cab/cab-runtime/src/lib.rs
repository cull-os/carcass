#![feature(let_chains)]

mod code;
pub use code::{
   ByteIndex,
   Code,
   ValueIndex,
};

mod compile;
pub use compile::oracle as compile_oracle;

mod island;
pub use island::Entry;

mod operation;
pub use operation::Operation;

mod scope;
pub use scope::{
   Local,
   LocalIndex,
   LocalName,
   Scope,
};

mod thunk;
pub use thunk::Thunk;

mod value;
pub use value::Value;
