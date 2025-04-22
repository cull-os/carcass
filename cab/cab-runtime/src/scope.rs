use std::ops;

use cab_why::Span;
use rustc_hash::FxHashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct LocalIndex(usize);

impl ops::Deref for LocalIndex {
   type Target = usize;

   fn deref(&self) -> &Self::Target {
      &self.0
   }
}

pub struct Local {
   name:  String,
   span:  Span,
   depth: usize,
   used:  bool,
}

pub struct Scope {
   locals:  Vec<Local>,
   by_name: FxHashMap<String, LocalIndex>,

   has_interpolated_reference: bool,
}
