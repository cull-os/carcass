use cab_why::Span;
use derive_more::Deref;
use smallvec::{
   SmallVec,
   smallvec,
};

#[derive(Deref, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct LocalIndex(usize);

#[derive(Debug, Clone, Eq)]
pub struct LocalName<'a>(SmallVec<&'a str, 4>);

impl PartialEq for LocalName<'_> {
   fn eq(&self, other: &Self) -> bool {
      self.0.len() <= 1 && self.0 == other.0
   }
}

impl<'this, 'a> TryInto<&'a str> for &'this LocalName<'a> {
   type Error = &'this [&'a str];

   fn try_into(self) -> Result<&'a str, Self::Error> {
      match self.0.len() {
         0 => Ok(""),
         1 => Ok(self.0[0]),
         _ => Err(&self.0),
      }
   }
}

impl<'a> LocalName<'a> {
   pub fn new(parts: SmallVec<&'a str, 4>) -> Self {
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

#[derive(Debug)]
pub enum LocalPosition<'this, 'a> {
   Known {
      index:  LocalIndex,
      scopes: &'this mut [Scope<'a>],
   },

   Unknown {
      name:   LocalName<'a>,
      scopes: &'this mut [Scope<'a>],
   },

   Undefined,
}

impl LocalPosition<'_, '_> {
   pub fn mark_used(&mut self) {
      match self {
         LocalPosition::Known { index, scopes } => {
            scopes
               .last_mut()
               .expect("known local must belong to a scope")
               .locals[**index]
               .used = true;
         },

         LocalPosition::Unknown { name, scopes } => {
            for scope in scopes.iter_mut().rev() {
               for (local_name, indices) in &scope.locals_by_name {
                  if !local_name.maybe_equals(name) {
                     continue;
                  }

                  let index = indices
                     .last()
                     .expect("by-name locals must have at least one item per entry");

                  scope.locals[**index].used = true;
               }
            }
         },

         LocalPosition::Undefined => panic!("tried to mark undefined reference as used"),
      }
   }
}

#[derive(Debug)]
pub struct Local<'a> {
   pub span: Span,
   pub name: LocalName<'a>,
   used:     bool,
}

#[derive(Debug)]
pub struct Scope<'a> {
   locals:         SmallVec<Local<'a>, 4>,
   locals_by_name: SmallVec<(LocalName<'a>, SmallVec<LocalIndex, 4>), 4>,
}

impl<'a> Scope<'a> {
   #[allow(clippy::new_without_default)]
   pub fn new() -> Self {
      Self {
         locals:         SmallVec::new(),
         locals_by_name: SmallVec::new(),
      }
   }

   pub fn push(&mut self, span: Span, name: LocalName<'a>) -> LocalIndex {
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
         Some((_, indices)) => indices.push(index),

         None => self.locals_by_name.push((name, smallvec![index])),
      }

      index
   }

   pub fn locate<'this>(
      scopes: &'this mut [Scope<'a>],
      name: &LocalName<'a>,
   ) -> LocalPosition<'this, 'a> {
      for (scope_index, scope) in scopes.iter().enumerate().rev() {
         for (local_name, indices) in &scope.locals_by_name {
            match () {
               _ if local_name == name => {
                  return LocalPosition::Known {
                     index:  *indices.last().unwrap(),
                     scopes: &mut scopes[scope_index..],
                  };
               },

               _ if local_name.maybe_equals(name) => {
                  return LocalPosition::Unknown {
                     name:   name.clone(),
                     scopes: &mut scopes[scope_index..],
                  };
               },

               _ => {},
            }
         }
      }

      LocalPosition::Undefined
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
