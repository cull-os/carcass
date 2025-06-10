#![allow(dead_code)]

use std::{
   collections::HashMap,
   sync::Arc,
};

use cab_span::Span;
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

impl From<Thunk> for Value {
   fn from(thunk: Thunk) -> Self {
      Value::Thunk(thunk)
   }
}

impl Thunk {
   #[must_use]
   pub fn suspended(span: Span, code: Code) -> Self {
      Self(
         RwLock::new(ThunkInner::Suspended {
            span,
            code,
            locals: HashMap::with_hasher(FxBuildHasher),
         })
         .into(),
      )
   }

   #[must_use]
   pub fn suspended_native(native: impl FnOnce() -> Value + Send + Sync + 'static) -> Self {
      Self(RwLock::new(ThunkInner::SuspendedNative(Box::new(native))).into())
   }
}
