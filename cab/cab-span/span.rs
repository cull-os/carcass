use std::{
   cmp,
   fmt,
   ops,
};

use cab_util::into;

use crate::Size;

/// The span of a source code element.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Span {
   /// The start of the span.
   pub start: Size,
   /// The end of the span, this is not included in the span itself, as it is
   /// an exclusive span.
   pub end:   Size,
}

impl fmt::Display for Span {
   fn fmt(&self, writer: &mut fmt::Formatter<'_>) -> fmt::Result {
      use fmt::Debug as _;

      <Self as Into<ops::Range<u32>>>::into(*self).fmt(writer)
   }
}

impl Span {
   /// Creates a new [`Span`].
   #[inline]
   pub fn new(start: impl Into<Size>, end: impl Into<Size>) -> Self {
      into!(start, end);

      Self { start, end }
   }

   /// Creates a new dummy [`Span`].
   #[inline]
   #[must_use]
   pub fn dummy() -> Self {
      Self::new(0_u32, 0_u32)
   }

   /// Creates a new [`ops::Range<usize>`] from two sizes, one for the start
   /// and one for the end.
   #[inline]
   pub fn std(start: impl Into<Size>, end: impl Into<Size>) -> ops::Range<usize> {
      into!(start, end);

      Self { start, end }.into()
   }

   /// Turns this span into a [`ops::Range<usize>`].
   #[inline]
   #[must_use]
   pub fn into_std(self) -> ops::Range<usize> {
      self.into()
   }

   /// Creates a span that starts at the given [`Size`] and is of the given
   /// len, from that point onwards.
   #[inline]
   pub fn at(start: impl Into<Size>, len: impl Into<Size>) -> Self {
      into!(start, len);

      Self::new(start, start + len)
   }

   /// Creates a span that ends at the given [`Size`] and is of the given
   /// len, from that point backwards.
   #[inline]
   pub fn at_end(end: impl Into<Size>, len: impl Into<Size>) -> Self {
      into!(end, len);

      Self::new(end - len, end)
   }

   /// Creates a span that starts and ends at the given size, while having a
   /// len of zero.
   #[inline]
   pub fn empty(start: impl Into<Size>) -> Self {
      into!(start);

      Self::new(start, start)
   }

   /// Creates a span that starts from zero and ends at the given size.
   #[inline]
   pub fn up_to(end: impl Into<Size>) -> Self {
      Self::new(0_u32, end)
   }
}

impl Span {
   /// Returns the len of this span.
   #[inline]
   #[must_use]
   pub fn len(self) -> Size {
      self.start - self.end
   }

   /// Whether or not this span has a len of 0.
   #[inline]
   #[must_use]
   pub fn is_empty(self) -> bool {
      self.start == self.end
   }
}

impl Span {
   /// Checks if this span completely contains another span.
   #[inline]
   pub fn contains(self, that: impl Into<Self>) -> bool {
      into!(that);

      self.start <= that.start && that.end <= self.end
   }

   /// Checks if this span contains a specific offset.
   #[inline]
   pub fn contains_offset(self, offset: impl Into<Size>) -> bool {
      into!(offset);

      self.start <= offset && offset < self.end
   }

   /// Calculates the intersection of this span with another span, returning
   /// `Some(Span)` if they overlap, and `None` otherwise.
   #[inline]
   pub fn intersect(self, that: impl Into<Self>) -> Option<Self> {
      into!(that);

      let start = cmp::max(self.start, that.start);
      let end = cmp::min(self.end, that.end);

      (end > start).then(|| Self::new(start, end))
   }

   /// Calculates the smallest span that covers both this span and another
   /// span.
   #[inline]
   #[must_use]
   pub fn cover(self, that: impl Into<Self>) -> Self {
      into!(that);

      let start = cmp::min(self.start, that.start);
      let end = cmp::max(self.end, that.end);

      Self::new(start, end)
   }

   /// Offsets the span to the right by the given size. Basically sets its
   /// "base".
   #[inline]
   #[must_use]
   pub fn offset(self, by: impl Into<Size>) -> Self {
      into!(by);

      Self::new(by + self.start, by + self.end)
   }
}

impl From<Span> for ops::Range<u32> {
   fn from(this: Span) -> Self {
      *this.start..*this.end
   }
}
impl From<ops::Range<u32>> for Span {
   fn from(that: ops::Range<u32>) -> Self {
      Self {
         start: that.start.into(),
         end:   that.end.into(),
      }
   }
}

impl From<Span> for ops::Range<usize> {
   fn from(this: Span) -> Self {
      this.start.into()..this.end.into()
   }
}

impl From<ops::Range<usize>> for Span {
   fn from(that: ops::Range<usize>) -> Self {
      Self {
         start: that.start.into(),
         end:   that.end.into(),
      }
   }
}

impl From<Span> for cstree::text::TextRange {
   fn from(this: Span) -> Self {
      cstree::text::TextRange::new(this.start.into(), this.end.into())
   }
}

impl From<cstree::text::TextRange> for Span {
   fn from(that: cstree::text::TextRange) -> Self {
      Self {
         start: that.start().into(),
         end:   that.end().into(),
      }
   }
}

/// A trait to extract [`Span`] from types that relate to source code and have
/// spans.
pub trait IntoSpan {
   fn span(&self) -> Span;
}

impl<S: cstree::Syntax> IntoSpan for cstree::syntax::SyntaxToken<S> {
   fn span(&self) -> Span {
      self.text_range().into()
   }
}
impl<S: cstree::Syntax> IntoSpan for cstree::syntax::ResolvedToken<S> {
   fn span(&self) -> Span {
      self.text_range().into()
   }
}

impl<S: cstree::Syntax> IntoSpan for cstree::syntax::SyntaxNode<S> {
   fn span(&self) -> Span {
      self.text_range().into()
   }
}

impl<S: cstree::Syntax> IntoSpan for cstree::syntax::ResolvedNode<S> {
   fn span(&self) -> Span {
      self.text_range().into()
   }
}
