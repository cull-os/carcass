#![allow(dead_code)]

use std::{
   collections::HashMap,
   sync::Arc,
};

use dup::Dupe;
use rustc_hash::{
   FxBuildHasher,
   FxHashMap,
};
use tokio::sync::RwLock;

use crate::{
   Code,
   Location,
   Value,
};

enum ThunkInner {
   Suspended {
      location: Location,
      code:     Code,
      locals:   FxHashMap<String, Value>,
   },

   SuspendedNative(Box<dyn FnOnce() -> Value + Send + Sync>),

   BlackHole {
      location:     Location,
      forced_at:    Location,
      suspended_at: Location,
   },

   Evaluated(Arc<Value>),
}

#[derive(Clone, Dupe)]
pub struct Thunk(Arc<RwLock<ThunkInner>>);

impl From<Thunk> for Value {
   fn from(thunk: Thunk) -> Self {
      Value::Thunk(thunk)
   }
}

impl Thunk {
   #[must_use]
   pub fn suspended(location: Location, code: Code) -> Self {
      Self(
         RwLock::new(ThunkInner::Suspended {
            location,
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
