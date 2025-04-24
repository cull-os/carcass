use std::{
   cell::RefCell,
   rc::Rc,
};

use cab_why::Span;
use derive_more::Deref;

#[derive(Deref, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct LocalIndex(usize);

#[derive(Debug, Clone, Eq)]
pub enum LocalName {
   Static(String),
   Interpolated(Vec<String>),
}

impl PartialEq for LocalName {
   fn eq(&self, other: &Self) -> bool {
      match (self, other) {
         (LocalName::Static(name), LocalName::Static(other_name)) => name == other_name,

         (LocalName::Static(name), LocalName::Interpolated(parts))
         | (LocalName::Interpolated(parts), LocalName::Static(name))
            if parts.len() <= 1 =>
         {
            parts.first().map_or("", |s| &**s) == name
         },

         (LocalName::Interpolated(parts), LocalName::Interpolated(other_parts))
            if parts.len() <= 1 =>
         {
            parts == other_parts
         },

         _ => false,
      }
   }
}

impl LocalName {
   fn maybe_equals(&self, other: &Self) -> bool {
      match (self, other) {
         (LocalName::Static(name), LocalName::Static(other_name)) => name == other_name,

         (LocalName::Interpolated(parts), LocalName::Interpolated(other_parts)) => {
            parts == other_parts
         },

         // Return true if the static identifier contains all `parts` in order, possibly with
         // arbitrary text in between from interpolation.
         (LocalName::Static(name), LocalName::Interpolated(parts))
         | (LocalName::Interpolated(parts), LocalName::Static(name)) => {
            let mut offset = 0;

            for part in parts {
               match name[offset..].find(part) {
                  Some(idx) => offset += idx + part.len(),

                  None => return false,
               }
            }

            offset == name.len()
         },
      }
   }
}

#[derive(Debug, Clone)]
pub enum LocalPosition {
   Known {
      scope: Rc<RefCell<Scope>>,
      index: LocalIndex,
   },

   Unknown {
      tree:            Rc<RefCell<Scope>>,
      tried_to_locate: LocalName,
   },

   Undefined,
}

impl LocalPosition {
   pub fn mark_used(&self) {
      match self {
         LocalPosition::Known { scope, index } => scope.borrow_mut().locals[**index].used = true,

         LocalPosition::Unknown {
            tree,
            tried_to_locate: name,
         } => {
            let mut scope = tree.clone();

            loop {
               {
                  let scope = &mut *scope.borrow_mut();

                  for (local_name, indexes) in &scope.locals_by_name {
                     if local_name.maybe_equals(name) {
                        let visible = indexes
                           .last()
                           .expect("by-name locals must have at least one item per entry");

                        scope.locals[**visible].used = true;
                     }
                  }
               }

               let Some(parent) = scope.borrow().parent.clone() else {
                  break;
               };

               scope = parent;
            }
         },

         LocalPosition::Undefined => panic!("tried to mark undefined reference as used"),
      }
   }
}

#[derive(Debug)]
pub struct Local {
   pub span: Span,
   pub name: LocalName,
   used:     bool,
}

#[derive(Debug)]
pub struct Scope {
   parent: Option<Rc<RefCell<Scope>>>,

   locals:         Vec<Local>,
   locals_by_name: Vec<(LocalName, Vec<LocalIndex>)>,
}

impl Default for Scope {
   fn default() -> Self {
      Self::root()
   }
}

impl Scope {
   pub fn root() -> Self {
      Self {
         parent:         None,
         locals:         Vec::new(),
         locals_by_name: Vec::new(),
      }
   }

   pub fn new(parent: &Rc<RefCell<Scope>>) -> Self {
      Self {
         parent:         Some(parent.clone()),
         locals:         Vec::new(),
         locals_by_name: Vec::new(),
      }
   }

   pub fn push(&mut self, span: Span, name: LocalName) -> LocalIndex {
      let index = LocalIndex(self.locals.len());
      self.locals.push(Local {
         span,
         name: name.clone(),
         used: false,
      });

      let slot = self
         .locals_by_name
         .iter_mut()
         .find(|(local_name, _)| local_name == &name);

      match slot {
         Some((_, indexes)) => indexes.push(index),

         None => self.locals_by_name.push((name, vec![index])),
      }

      index
   }

   pub fn locate(this: &Rc<RefCell<Self>>, name: &LocalName) -> LocalPosition {
      for (local_name, indexes) in &this.borrow().locals_by_name {
         match () {
            _ if local_name == name => {
               return LocalPosition::Known {
                  scope: this.clone(),
                  index: *indexes.last().expect(""),
               };
            },

            _ if local_name.maybe_equals(name) => {
               return LocalPosition::Unknown {
                  tree:            this.clone(),
                  tried_to_locate: name.clone(),
               };
            },

            _ => {},
         }
      }

      this
         .borrow()
         .parent
         .as_ref()
         .map_or(LocalPosition::Undefined, |parent| {
            Scope::locate(parent, name)
         })
   }

   pub fn finish(&self) -> impl Iterator<Item = &Local> {
      self.locals.iter().filter(|local| {
         let unused = !local.used;

         unused && {
            let ignored = match &local.name {
               LocalName::Static(name) => name.starts_with('_'),
               LocalName::Interpolated(items) => {
                  items.first().is_some_and(|first| first.starts_with('_'))
               },
            };

            !ignored
         }
      })
   }
}

#[cfg(test)]
mod tests {
   use super::*;

   #[test]
   fn local_name_equality() {
      todo!();
   }
}
