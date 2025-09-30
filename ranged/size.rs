use std::ops;

use cab_util::into;
use derive_more::{
   Deref,
   DerefMut,
};
use dup::Dupe;

/// Byte len of a source code element.
#[derive(Deref, DerefMut, Debug, Clone, Dupe, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Size(u32);

impl Size {
   // Creates a new [`Size`].
   #[inline]
   pub fn new(size: impl Into<Size>) -> Self {
      into!(size);
      size
   }
}

impl<I: Into<Self>> ops::Add<I> for Size {
   type Output = Self;

   fn add(self, that: I) -> Self::Output {
      into!(that);

      Self(*self + *that)
   }
}

impl<I: Into<Self>> ops::Sub<I> for Size {
   type Output = Self;

   #[track_caller]
   fn sub(self, that: I) -> Self::Output {
      into!(that);

      Self(*self - *that)
   }
}

impl<I> ops::AddAssign<I> for Size
where
   Self: ops::Add<I, Output = Self>,
{
   fn add_assign(&mut self, rhs: I) {
      *self = *self + rhs;
   }
}

impl<I> ops::SubAssign<I> for Size
where
   Self: ops::Sub<I, Output = Self>,
{
   #[track_caller]
   fn sub_assign(&mut self, rhs: I) {
      *self = *self - rhs;
   }
}

impl From<Size> for u32 {
   fn from(this: Size) -> Self {
      *this
   }
}

impl From<u32> for Size {
   fn from(that: u32) -> Self {
      Self(that)
   }
}

impl From<Size> for usize {
   fn from(this: Size) -> Self {
      *this as usize
   }
}

impl From<usize> for Size {
   fn from(that: usize) -> Self {
      Self(u32::try_from(that).expect("usize must fit in u32"))
   }
}

#[cfg(feature = "cstree")]
impl From<Size> for cstree::text::TextSize {
   fn from(this: Size) -> Self {
      cstree::text::TextSize::new(*this)
   }
}

#[cfg(feature = "cstree")]
impl From<cstree::text::TextSize> for Size {
   fn from(that: cstree::text::TextSize) -> Self {
      into!(that);

      Self(that)
   }
}

/// A trait to extract [`Size`] from types that relate to source code and have
/// sizes.
pub trait IntoSize {
   fn size(&self) -> Size;
}

impl IntoSize for u8 {
   fn size(&self) -> Size {
      1_u32.into()
   }
}

impl IntoSize for char {
   fn size(&self) -> Size {
      self.len_utf8().into()
   }
}

impl IntoSize for str {
   fn size(&self) -> Size {
      self.len().into()
   }
}

impl IntoSize for String {
   fn size(&self) -> Size {
      self.len().into()
   }
}

#[cfg(feature = "cstree")]
impl<I: cstree::interning::Resolver + ?Sized, S: cstree::Syntax> IntoSize
   for cstree::text::SyntaxText<'_, '_, I, S>
{
   fn size(&self) -> Size {
      self.len().into()
   }
}
