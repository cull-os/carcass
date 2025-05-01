#![allow(dead_code)]

use std::sync::Arc;

use rpds::HashTrieMapSync as HashTrieMap;
use rustc_hash::FxBuildHasher;

use super::Value;

#[derive(Clone)]
pub struct Attributes(HashTrieMap<Arc<str>, Value, FxBuildHasher>);

impl From<Attributes> for Value {
   fn from(val: Attributes) -> Self {
      Value::Attributes(val)
   }
}

impl Attributes {
   #[allow(clippy::new_without_default)]
   pub fn new() -> Self {
      Self(HashTrieMap::new_with_hasher_and_ptr_kind(FxBuildHasher))
   }
}
