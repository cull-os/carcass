use std::{
   collections::HashMap,
   sync::Arc,
};

use cab_why::Span;
use rustc_hash::{
   FxBuildHasher,
   FxHashMap,
};

use crate::{
   Code,
   Value,
};

pub enum Thunk {
   Suspended {
      span:   Span,
      code:   Code,
      locals: FxHashMap<String, Value>,
   },

   SuspendedNative(Box<dyn FnOnce() -> Value + Send + Sync>),

   BlackHole {
      span:         Span,
      forced_at:    Span,
      suspended_at: Span,
   },

   Evaluated(Arc<Value>),
}

impl Thunk {
   pub fn suspended(span: Span, code: Code) -> Self {
      Self::Suspended {
         span,
         code,
         locals: HashMap::with_hasher(FxBuildHasher),
      }
   }

   pub fn suspended_native(native: impl FnOnce() -> Value + Send + Sync + 'static) -> Self {
      Self::SuspendedNative(Box::new(native))
   }
}
