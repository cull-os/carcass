use std::ops;

use crate::into;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Size(u32);

impl Size {
    pub fn new(size: impl Into<Size>) -> Self {
        into!(size);
        size
    }
}

// OPERATIONS

impl<I: Into<Self>> ops::Add<I> for Size {
    type Output = Self;

    fn add(self, that: I) -> Self::Output {
        Self(*self + *that.into())
    }
}

impl<I: Into<Self>> ops::Sub<I> for Size {
    type Output = Self;

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
    fn sub_assign(&mut self, rhs: I) {
        *self = *self - rhs
    }
}

// U32 CONVERSIONS

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

// USIZE CONVERSIONS

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

// TEXTSIZE CONVERSIONS

impl From<Size> for cstree::text::TextSize {
    fn from(this: Size) -> Self {
        cstree::text::TextSize::new(*this)
    }
}

impl From<cstree::text::TextSize> for Size {
    fn from(that: cstree::text::TextSize) -> Self {
        Self(that.into())
    }
}

// SIZEABLE

pub trait Sizeable {
    fn size(&self) -> Size;
}

impl Sizeable for u8 {
    fn size(&self) -> Size {
        1u32.into()
    }
}

impl Sizeable for char {
    fn size(&self) -> Size {
        self.len_utf8().into()
    }
}

impl Sizeable for &'_ str {
    fn size(&self) -> Size {
        self.len().into()
    }
}

impl Sizeable for &'_ String {
    fn size(&self) -> Size {
        self.len().into()
    }
}
