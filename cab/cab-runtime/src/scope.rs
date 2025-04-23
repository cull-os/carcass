use std::{
   cell::RefCell,
   ops,
   rc::Rc,
};

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

#[derive(Debug, Clone)]
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

#[derive(Debug)]
pub struct Local {
   span: Span,
   name: LocalName,
   used: bool,
}

#[derive(Debug)]
pub struct Scope {
   pub parent:      Option<Rc<RefCell<Scope>>>,
   pub locals:      Vec<Local>,
   pub by_name:     FxHashMap<String, LocalIndex>,
   pub has_dynamic: bool,
}

impl Default for Scope {
   fn default() -> Self {
      Self::root()
   }
}

impl Scope {
   pub fn root() -> Self {
      Self {
         parent:      None,
         locals:      Vec::new(),
         by_name:     FxHashMap::with_hasher(FxBuildHasher),
         has_dynamic: false,
      }
   }

   pub fn new(parent: &Rc<RefCell<Scope>>) -> Self {
      Self {
         parent:      Some(Rc::clone(parent)),
         locals:      Vec::new(),
         by_name:     FxHashMap::with_hasher(FxBuildHasher),
         has_dynamic: false,
      }
   }

   pub fn resolve(
      this: &Rc<RefCell<Self>>,
      name: &str,
   ) -> Option<(Rc<RefCell<Scope>>, Option<LocalIndex>)> {
      if this.borrow().has_dynamic {
         return Some((this.clone(), None));
      }

      if let Some(index) = this.borrow().by_name.get(name) {
         return Some((this.clone(), Some(*index)));
      }

      this
         .borrow()
         .parent
         .as_ref()
         .and_then(|parent| Scope::resolve(parent, name))
   }

   pub fn is_self_contained(&self) -> bool {
      self.locals.iter().enumerate().all(|(index, local)| {
         // Inclusive range because `@foo = foo` is possible.
         let defined_locally = self.locals[..=index]
            .iter()
            .any(|defined| local.name == defined.name);

         defined_locally || {
            let LocalName::Static(name) = &local.name else {
               unreachable!()
            };

            let defined_externally = self
               .parent
               .as_ref()
               .and_then(|parent| Scope::resolve(parent, name))
               .is_some();

            // Not defined externally, which means it is not defined anywhere.
            !defined_externally
         }
      })
   }

   pub fn push(&mut self, span: Span, name: LocalName) -> LocalIndex {
      let index = LocalIndex(self.locals.len());
      self.locals.push(Local {
         span,
         name: name.clone(),
         used: false,
      });

      match name {
         LocalName::Static(name) => {
            self.by_name.insert(name, index);
         },

         LocalName::Dynamic => {
            self.has_dynamic = true;
         },
      }

      index
   }

   pub fn mark_used(&mut self, index: LocalIndex) {
      self.locals[*index].used = true;
   }

   pub fn mark_all_used(&mut self) {
      for index in self.by_name.values().copied() {
         self.locals[*index].used = true;
      }
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
