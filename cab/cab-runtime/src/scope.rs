use std::ops;

use cab_why::Span;
use rustc_hash::{
   FxBuildHasher,
   FxHashMap,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct LocalIndex(usize);

impl ops::Deref for LocalIndex {
   type Target = usize;

   fn deref(&self) -> &Self::Target {
      &self.0
   }
}

#[derive(Debug)]
pub enum LocalName {
   Static(String),
   Dynamic,
}

impl PartialEq for LocalName {
   fn eq(&self, other: &Self) -> bool {
      match self {
         LocalName::Static(name) if let LocalName::Static(other_name) = other => name == other_name,

         _ => false,
      }
   }
}

pub struct Local {
   span:     Span,
   name:     LocalName,
   pub used: bool,
}

pub struct Scope {
   pub locals:  Vec<Local>,
   pub by_name: FxHashMap<String, LocalIndex>,
}

impl Scope {
   #[allow(clippy::new_without_default)]
   pub fn new() -> Self {
      Self {
         locals:  Vec::new(),
         by_name: FxHashMap::with_hasher(FxBuildHasher),
      }
   }

   pub fn is_self_contained(&self) -> bool {
      self.locals.iter().enumerate().all(|(index, local)| {
         // Inclusive range because `@foo = foo` is possible.
         self.locals[..=index]
            .iter()
            .any(|defined| local.name == defined.name)
      })
   }
}

#[cfg(test)]
mod tests {
   use super::*;

   #[test]
   fn local_name_equality() {
      assert_eq!(
         LocalName::Static("foo".to_owned()),
         LocalName::Static("foo".to_owned()),
      );

      assert_ne!(
         LocalName::Static("a".to_owned()),
         LocalName::Static("b".to_owned())
      );

      assert_ne!(LocalName::Static("foo".to_owned()), LocalName::Dynamic);

      assert_ne!(LocalName::Dynamic, LocalName::Dynamic);
   }
}
