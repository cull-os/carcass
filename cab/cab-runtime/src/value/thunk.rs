use std::{
   collections::HashMap,
   sync::Arc,
};

use cab_why::Span;
use rustc_hash::{
   FxBuildHasher,
   FxHashMap,
};
use tokio::sync::RwLock;

use crate::{
   Code,
   Value,
};

enum ThunkInner {
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

#[derive(Clone)]
pub struct Thunk(Arc<RwLock<ThunkInner>>);

impl Into<Value> for Thunk {
   fn into(self) -> Value {
      Value::Thunk(self)
   }
}

impl Thunk {
   pub fn suspended(span: Span, code: Code) -> Self {
      Self(Arc::new(RwLock::new(ThunkInner::Suspended {
         span,
         code,
         locals: HashMap::with_hasher(FxBuildHasher),
      })))
   }

   pub fn suspended_native(native: impl FnOnce() -> Value + Send + Sync + 'static) -> Self {
      Self(Arc::new(RwLock::new(ThunkInner::SuspendedNative(
         Box::new(native),
      ))))
   }
}
