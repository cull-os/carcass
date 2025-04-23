use std::sync::{
   Arc,
   Mutex,
};

use indexmap::IndexMap;
use rustc_hash::FxBuildHasher;

use crate::{
   Code,
   Thunk,
};

#[warn(variant_size_differences)]
#[derive(Clone)]
pub enum Value {
   Nil,

   Path(String),

   Rune(char),
   Integer(num::BigInt),
   Float(f64),

   Attributes(Arc<IndexMap<String, Value, FxBuildHasher>>),

   Bind(String),
   Identifier(String),

   Thunk(Arc<Mutex<Thunk>>),
   Blueprint(Arc<Code>),
}
