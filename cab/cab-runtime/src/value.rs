use std::sync::{
   Arc,
   Mutex,
};

use rpds::HashTrieMapSync as HashTrieMap;
use rustc_hash::FxBuildHasher;

use crate::{
   Code,
   Thunk,
};

#[warn(variant_size_differences)]
#[derive(Clone)]
pub enum Value {
   Nil,

   Rune(char),
   Integer(num::BigInt),
   Float(f64),

   Attributes(HashTrieMap<String, Value, FxBuildHasher>),

   IslandHeader(Arc<str>),
   Path(Arc<str>),

   Bind(Arc<str>),
   Identifier(Arc<str>),

   Thunk(Arc<Mutex<Thunk>>),
   Blueprint(Arc<Code>),
}
