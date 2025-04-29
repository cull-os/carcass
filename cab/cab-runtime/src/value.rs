use std::sync::Arc;

use rpds::HashTrieMapSync as HashTrieMap;
use rustc_hash::FxBuildHasher;
use tokio::sync::RwLock;

use crate::{
   Code,
   Thunk,
};

#[warn(variant_size_differences)]
#[derive(Clone)]
pub enum Value {
   Nil,
   Cons(Arc<Value>, Arc<Value>),

   Attributes(HashTrieMap<Arc<str>, Value, FxBuildHasher>),

   Path(Arc<str>),

   Bind(Arc<str>),
   Reference(Arc<str>),

   Rune(char),
   Integer(num::BigInt),
   Float(f64),

   Thunk(Arc<RwLock<Thunk>>),
   Blueprint(Arc<Code>),
}
