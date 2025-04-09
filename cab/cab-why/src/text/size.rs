use std::ops;

use crate::into;

/// Byte len of a source code element.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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
        Self(*self + *that.into())
    }
}

impl<I: Into<Self>> ops::Sub<I> for Size {
    type Output = Self;

    #[track_caller]
    fn sub(self, that: I) -> Self::Output {
        Self(*self - *that.into())
    }
}

impl<I> ops::AddAssign<I> for Size
where
    Self: ops::Add<I, Output = Self>,
{
    fn add_assign(&mut self, rhs: I) {
        *self = *self + rhs
    }
}

impl<I> ops::SubAssign<I> for Size
where
    Self: ops::Sub<I, Output = Self>,
{
    #[track_caller]
    fn sub_assign(&mut self, rhs: I) {
        *self = *self - rhs
    }
}

impl ops::Deref for Size {
    type Target = u32;

    fn deref(&self) -> &Self::Target {
        &self.0
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
        Self(that.try_into().expect("size too big"))
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
        Self(that.into())
    }
}

/// A trait to extract [`Size`] from types that relate to source code and have
/// sizes.
pub trait IntoSize {
    fn size(&self) -> Size;
}

impl IntoSize for u8 {
    fn size(&self) -> Size {
        1u32.into()
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
