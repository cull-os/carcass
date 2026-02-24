use std::sync::atomic;

use dup::Dupe;
use rpds::ListSync as List;

use crate::{
   Value,
   value,
};

const EXPECT_SCOPE: &str = "must have at least once scope";

#[derive(Clone, Copy, Dupe, PartialEq, Eq, Hash)]
pub struct ScopeId(u64);

impl ScopeId {
   #[must_use]
   pub fn new() -> Self {
      static NEXT: atomic::AtomicU64 = atomic::AtomicU64::new(0);

      Self(NEXT.fetch_add(1, atomic::Ordering::Relaxed))
   }
}

#[derive(Clone, Dupe)]
pub struct Scope {
   id:         ScopeId,
   attributes: value::Attributes,
}

impl From<&value::Attributes> for Scope {
   fn from(attributes: &value::Attributes) -> Self {
      Self {
         id:         ScopeId::new(),
         attributes: attributes.dupe(),
      }
   }
}

impl Scope {
   #[must_use]
   pub fn new() -> Self {
      Self {
         id:         ScopeId::new(),
         attributes: value::attributes::new! {},
      }
   }

   #[must_use]
   pub fn id(&self) -> ScopeId {
      self.id
   }

   #[must_use]
   pub fn attributes(&self) -> &value::Attributes {
      &self.attributes
   }

   #[must_use]
   pub fn insert(&self, key: value::SString, value: Value) -> Self {
      Self {
         id:         self.id,
         attributes: self.attributes.insert(key, value),
      }
   }

   #[must_use]
   pub fn merge(&self, with: &value::Attributes) -> Self {
      Self {
         id:         self.id,
         attributes: self.attributes.merge(with),
      }
   }
}

#[derive(Clone, Dupe)]
pub struct Scopes(List<Scope>);

impl Scopes {
   #[must_use]
   pub fn new() -> Self {
      Self(List::new_sync())
   }

   #[must_use]
   pub fn tip(&self) -> Option<&Scope> {
      self.0.first()
   }

   pub fn iter(&self) -> impl Iterator<Item = &Scope> {
      self.0.iter()
   }

   #[must_use]
   pub fn get(&self, key: &value::SString) -> Option<&Value> {
      self.iter().find_map(|scope| scope.attributes.get(key))
   }

   #[must_use]
   pub fn push(&self, scope: Scope) -> Self {
      Self(self.0.push_front(scope))
   }

   #[must_use]
   pub fn pop(&self) -> Option<Self> {
      self.0.drop_first().map(Self)
   }

   #[must_use]
   pub fn merge_tip_from(&self, with: &Self) -> Self {
      self.pop().expect(EXPECT_SCOPE).push(
         self
            .tip()
            .expect(EXPECT_SCOPE)
            .merge(with.tip().expect(EXPECT_SCOPE).attributes()),
      )
   }
}
