use std::sync::Arc;

use indexmap::IndexMap;
use rustc_hash::FxHasher;

use crate::Code;

#[warn(variant_size_differences)]
#[derive(Clone)]
pub enum Constant {
   Nil,

   Path(String),

   Rune(char),
   Integer(num::BigInt),
   Float(f64),

   Attributes(Arc<IndexMap<String, Constant, FxHasher>>),

   Blueprint(Arc<Code>),
}
