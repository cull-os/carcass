use dup::Dupe;
use ranged::Span;
use ust::{
   style::StyledExt as _,
   terminal::tag,
};

use crate::value;

#[derive(Clone, Dupe)]
pub struct Location {
   pub path: value::Path,
   pub span: Span,
}

impl tag::DisplayTags for Location {
   fn display_tags<'a>(&'a self, tags: &mut tag::Tags<'a>) {
      // FIXME: No. Use a report formatter or something, because this is not row-col.
      self.path.display_tags(tags);
      tags.write(":");
      tags.write(
         self
            .span
            .start
            .to_string()
            .style(ust::STYLE_HEADER_POSITION),
      );
      tags.write(":");
      tags.write(self.span.end.to_string().style(ust::STYLE_HEADER_POSITION));
   }
}

impl Location {
   #[must_use]
   pub fn new(path: value::Path, span: Span) -> Self {
      Self { path, span }
   }
}
