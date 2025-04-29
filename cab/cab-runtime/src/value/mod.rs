use std::sync::Arc;

mod attributes;
pub use attributes::Attributes;

mod thunk;
pub use thunk::Thunk;

use crate::Code;

#[warn(variant_size_differences)]
#[derive(Clone)]
pub enum Value {
   Nil,
   Cons(Arc<Value>, Arc<Value>),

   Attributes(Attributes),

   Path(Arc<str>),

   Bind(Arc<str>),
   Reference(Arc<str>),

   Rune(char),
   Integer(num::BigInt),
   Float(f64),

   Thunk(Thunk),
   Blueprint(Arc<Code>),
}
