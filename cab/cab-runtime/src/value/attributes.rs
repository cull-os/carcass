use std::sync::Arc;

use rpds::HashTrieMapSync as HashTrieMap;
use rustc_hash::FxBuildHasher;

use super::Value;

#[derive(Clone)]
pub struct Attributes(HashTrieMap<Arc<str>, Value, FxBuildHasher>);

impl Into<Value> for Attributes {
   fn into(self) -> Value {
      Value::Attributes(self)
   }
}

impl Attributes {
   pub fn new() -> Self {
      Self(HashTrieMap::new_with_hasher_and_ptr_kind(FxBuildHasher))
   }
}
