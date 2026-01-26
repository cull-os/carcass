use dup::Dupe;
use ranged::Span;

use crate::value;

#[derive(Clone, Dupe)]
pub struct Location {
   pub path: value::Path,
   pub span: Span,
}

impl Location {
   pub fn new(path: value::Path, span: Span) -> Self {
      Self { path, span }
   }
}
