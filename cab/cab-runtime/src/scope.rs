use std::{
   cell::RefCell,
   rc::Rc,
};

use cab_why::Span;
use derive_more::Deref;

#[derive(Deref, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct LocalIndex(usize);

#[derive(Debug, Clone, Eq)]
pub struct LocalName(Vec<String>);

impl PartialEq for LocalName {
   fn eq(&self, other: &Self) -> bool {
      self.0.len() <= 1 && self.0 == other.0
   }
}

impl<'a> TryInto<&'a str> for &'a LocalName {
   type Error = &'a [String];

   fn try_into(self) -> Result<&'a str, Self::Error> {
      match self.0.len() {
         0 => Ok(""),
         1 => Ok(&self.0[0]),
         _ => Err(&self.0),
      }
   }
}

impl LocalName {
   pub fn new(parts: Vec<String>) -> Self {
      Self(parts)
   }

   fn maybe_equals(&self, other: &Self) -> bool {
      match (
         TryInto::<&str>::try_into(self),
         TryInto::<&str>::try_into(other),
      ) {
         (Ok(name), Ok(other_name)) => name == other_name,

         // Return true if `name` *can* contain all `parts` in order,
         // possibly with arbitrary text in between.
         //
         // Can never return false positives, aka can never return true for two
         // names that will *never* match.
         (Ok(name), Err(parts)) | (Err(parts), Ok(name)) => {
            let mut offset = 0;

            for part in &parts[..parts.len() - 1] {
               match name[offset..].find(part) {
                  Some(idx) => offset += idx + part.len(),

                  None => return false,
               }
            }

            let last = parts.last().expect("len was statically checked");

            name.ends_with(last) && name.len() - last.len() >= offset
         },

         (Err(parts), Err(other_parts)) => {
            ({
               let first = &parts[0];
               let other_first = &other_parts[0];

               first.starts_with(other_first) || other_first.starts_with(first)
            } && {
               let last = parts.last().expect("len was statically checked");
               let other_last = other_parts.last().expect("len was statically checked");

               last.starts_with(other_last) || other_last.starts_with(last)
            })
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
      haystack: Rc<RefCell<Scope>>,
      needle:   LocalName,
   },

   Undefined,
}

impl LocalPosition {
   pub fn mark_used(&self) {
      match self {
         LocalPosition::Known { scope, index } => scope.borrow_mut().locals[**index].used = true,

         LocalPosition::Unknown { haystack, needle } => {
            let mut haystack = haystack.clone();

            loop {
               {
                  let scope = &mut *haystack.borrow_mut();

                  for (local_name, indexes) in &scope.locals_by_name {
                     if local_name.maybe_equals(needle) {
                        let visible = indexes
                           .last()
                           .expect("by-name locals must have at least one item per entry");

                        scope.locals[**visible].used = true;
                     }
                  }
               }

               let Some(parent) = haystack.borrow().parent.clone() else {
                  break;
               };

               haystack = parent;
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
                  haystack: this.clone(),
                  needle:   name.clone(),
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
            let ignored = &local
               .name
               .0
               .first()
               .is_some_and(|name| name.starts_with('_'));

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
