use std::borrow::Cow;

use cab_span::Span;
use derive_more::Deref;
use smallvec::{
   SmallVec,
   smallvec,
};

const GLOBALS: &[&str] = &["false", "true"];

const BY_NAME_EXPECT: &str = "by-name locals must have at least one item per entry";

#[derive(Deref, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct LocalIndex(usize);

#[derive(Debug, Clone, Eq)]
pub struct LocalName<'a>(SmallVec<Cow<'a, str>, 4>);

impl PartialEq for LocalName<'_> {
   fn eq(&self, other: &Self) -> bool {
      self.0.len() <= 1 && self.0 == other.0
   }
}

impl LocalName<'_> {
   pub fn maybe_eq(&self, other: &Self) -> bool {
      match (
         TryInto::<&str>::try_into(self),
         TryInto::<&str>::try_into(other),
      ) {
         (Ok(name), Ok(other_name)) => name == other_name,

         // Return true if `name` *can* contain all `segments` in order,
         // possibly with arbitrary text in between.
         //
         // Can never return false positives, aka can never return true for two
         // names that will *never* match.
         (Ok(name), Err(segments)) | (Err(segments), Ok(name)) => {
            let mut offset = 0;

            for segment in &segments[..segments.len() - 1] {
               match name[offset..].find(segment.as_ref()) {
                  Some(idx) => offset += idx + segment.len(),

                  None => return false,
               }
            }

            let last = segments.last().expect("len was statically checked");

            name.ends_with(last.as_ref()) && name.len() - last.len() >= offset
         },

         (Err(segments), Err(other_segments)) => {
            ({
               let first = &segments[0];
               let other_first = &other_segments[0];

               first.starts_with(other_first.as_ref()) || other_first.starts_with(first.as_ref())
            } && {
               let last = segments.last().expect("len was statically checked");
               let other_last = other_segments.last().expect("len was statically checked");

               last.starts_with(other_last.as_ref()) || other_last.starts_with(last.as_ref())
            })
         },
      }
   }
}

impl<'a> TryInto<&'a str> for &'a LocalName<'a> {
   type Error = &'a [Cow<'a, str>];

   fn try_into(self) -> Result<&'a str, Self::Error> {
      match self.0.len() {
         0 => Ok(""),
         1 => Ok(&self.0[0]),
         _ => Err(&self.0),
      }
   }
}

impl<'a> LocalName<'a> {
   pub fn new(segments: SmallVec<impl Into<Cow<'a, str>>, 4>) -> Self {
      Self(segments.into_iter().map(Into::into).collect())
   }

   pub fn plain(s: &'a str) -> Self {
      Self::new(smallvec![s])
   }

   pub fn wildcard() -> Self {
      Self::new(smallvec!["", ""])
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
      match *self {
         LocalPosition::Known {
            index,
            ref mut scopes,
         } => {
            scopes
               .first_mut()
               .expect("known local must belong to a scope")
               .locals[*index]
               .used = true;
         },

         LocalPosition::Unknown {
            ref name,
            ref mut scopes,
         } => {
            for scope in scopes.iter_mut().rev() {
               #[expect(clippy::pattern_type_mismatch)]
               for (local_name, indices) in &scope.locals_by_name {
                  if !local_name.maybe_eq(name) {
                     continue;
                  }

                  let index = indices.last().expect(BY_NAME_EXPECT);

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

impl Scope<'_> {
   pub fn new() -> Self {
      Self {
         locals:         SmallVec::new(),
         locals_by_name: SmallVec::new(),
      }
   }

   pub fn global() -> Self {
      let mut this = Self::new();

      for global in GLOBALS {
         this.push(Span::dummy(), LocalName::plain(global));
      }

      this
   }
}

impl<'a> Scope<'a> {
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
         .find(|&&mut (ref local_name, _)| local_name == &name);

      match slot {
         Some(&mut (_, ref mut indices)) => indices.push(index),

         None => self.locals_by_name.push((name, smallvec![index])),
      }

      index
   }

   pub fn locate<'this>(
      scopes: &'this mut [Scope<'a>],
      name: &LocalName<'a>,
   ) -> LocalPosition<'this, 'a> {
      for (scope_index, scope) in scopes.iter().enumerate().rev() {
         #[expect(clippy::pattern_type_mismatch)]
         for (local_name, indices) in &scope.locals_by_name {
            match () {
               () if local_name.eq(name) => {
                  return LocalPosition::Known {
                     index:  *indices.last().expect(BY_NAME_EXPECT),
                     scopes: &mut scopes[scope_index..],
                  };
               },

               () if local_name.maybe_eq(name) => {
                  return LocalPosition::Unknown {
                     name:   name.clone(),
                     scopes: &mut scopes[scope_index..],
                  };
               },

               () => {},
            }
         }
      }

      LocalPosition::Undefined
   }

   pub fn is_user_defined(scopes: &mut [Scope<'a>], name: &'a str) -> bool {
      !matches!(
         Self::locate(&mut scopes[1..], &LocalName::plain(name)),
         LocalPosition::Undefined
      )
   }

   pub fn is_empty(&self) -> bool {
      self.locals.is_empty()
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
