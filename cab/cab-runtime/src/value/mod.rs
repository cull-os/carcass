use std::sync::Arc;

mod attributes;
pub use attributes::Attributes;

mod path;
pub use path::{
   Path,
   Root,
};

mod thunk;
pub use thunk::Thunk;

use crate::Code;

#[warn(variant_size_differences)]
#[derive(Clone)]
pub enum Value {
   Boolean(bool),

   Nil,
   Cons(Arc<Value>, Arc<Value>),

   Attributes(Attributes),

   Path(Path),

   Bind(Arc<str>),
   Reference(Arc<str>),
   String(Arc<str>),

   Rune(char),
   Integer(num::BigInt),
   Float(f64),

   Thunk(Thunk),
   Blueprint(Arc<Code>),
}
