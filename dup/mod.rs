//! Cheap clone trait for distinguishing expensive clones from things that
//! should have been Copy.

use std::iter;

#[doc(inline)] pub use dup_macros::Dupe;

pub trait Dupe: Clone {
   #[inline]
   #[must_use]
   fn dupe(&self) -> Self {
      self.clone()
   }
}

pub trait OptionDupedExt {
   type Item;

   fn duped(self) -> Option<Self::Item>;
}

impl<T: Dupe> OptionDupedExt for Option<&T> {
   type Item = T;

   fn duped(self) -> Option<T> {
      self.map(Dupe::dupe)
   }
}

pub trait IteratorDupedExt {
   fn duped(self) -> iter::Cloned<Self>
   where
      Self: Sized;
}

impl<'a, I: Iterator<Item = &'a (impl Dupe + 'a)>> IteratorDupedExt for I {
   fn duped(self) -> iter::Cloned<Self> {
      self.cloned()
   }
}

mod copy {
   use std::{
      any,
      marker,
      net,
      num,
      thread,
      time,
   };

   use super::Dupe;

   impl Dupe for bool {}

   impl Dupe for char {}

   impl Dupe for u8 {}

   impl Dupe for u16 {}

   impl Dupe for u32 {}

   impl Dupe for u64 {}

   impl Dupe for u128 {}

   impl Dupe for usize {}

   impl Dupe for i8 {}

   impl Dupe for i16 {}

   impl Dupe for i32 {}

   impl Dupe for i64 {}

   impl Dupe for i128 {}

   impl Dupe for isize {}

   impl Dupe for f32 {}

   impl Dupe for f64 {}

   impl Dupe for num::NonZeroU8 {}

   impl Dupe for num::NonZeroU16 {}

   impl Dupe for num::NonZeroU32 {}

   impl Dupe for num::NonZeroU64 {}

   impl Dupe for num::NonZeroU128 {}

   impl Dupe for num::NonZeroUsize {}

   impl Dupe for num::NonZeroI8 {}

   impl Dupe for num::NonZeroI16 {}

   impl Dupe for num::NonZeroI32 {}

   impl Dupe for num::NonZeroI64 {}

   impl Dupe for num::NonZeroI128 {}

   impl Dupe for num::NonZeroIsize {}

   impl Dupe for any::TypeId {}

   impl Dupe for marker::PhantomPinned {}

   impl Dupe for net::Ipv4Addr {}

   impl Dupe for net::Ipv6Addr {}

   impl Dupe for net::SocketAddrV4 {}

   impl Dupe for net::SocketAddrV6 {}

   impl Dupe for thread::ThreadId {}

   impl Dupe for time::Instant {}

   impl Dupe for time::SystemTime {}

   impl Dupe for time::Duration {}

   impl<T: ?Sized> Dupe for marker::PhantomData<T> {}
}

#[rustfmt::skip]
mod pointers {
   use std::{
      cell,
      mem,
      rc,
      sync,
   };

   use super::Dupe;

   impl<A: ?Sized> Dupe for &A {}

   impl<A: ?Sized> Dupe for *const A {}

   impl<A: ?Sized> Dupe for *mut A {}

   impl<A: ?Sized> Dupe for sync::Arc<A> {}

   impl<A: ?Sized> Dupe for sync::Weak<A> {}

   impl<A: ?Sized> Dupe for rc::Rc<A> {}

   impl<A: ?Sized> Dupe for rc::Weak<A> {}

   impl<A: Copy> Dupe for cell::Cell<A> {}

   impl<A: Dupe> Dupe for mem::ManuallyDrop<A> {}

   impl<R> Dupe for fn() -> R {}
   impl<R, A> Dupe for fn(A) -> R {}
   impl<R, A, B> Dupe for fn(A, B) -> R {}
   impl<R, A, B, C> Dupe for fn(A, B, C) -> R {}
   impl<R, A, B, C, D> Dupe for fn(A, B, C, D) -> R {}
   impl<R, A, B, C, D, E> Dupe for fn(A, B, C, D, E) -> R {}
   impl<R, A, B, C, D, E, F> Dupe for fn(A, B, C, D, E, F) -> R {}
   impl<R, A, B, C, D, E, F, G> Dupe for fn(A, B, C, D, E, F, G) -> R {}
   impl<R, A, B, C, D, E, F, G, H> Dupe for fn(A, B, C, D, E, F, G, H) -> R {}
   impl<R, A, B, C, D, E, F, G, H, I> Dupe for fn(A, B, C, D, E, F, G, H, I) -> R {}
   impl<R, A, B, C, D, E, F, G, H, I, J> Dupe for fn(A, B, C, D, E, F, G, H, I, J) -> R {}
   impl<R, A, B, C, D, E, F, G, H, I, J, K> Dupe for fn(A, B, C, D, E, F, G, H, I, J, K) -> R {}
   impl<R, A, B, C, D, E, F, G, H, I, J, K, L> Dupe for fn(A, B, C, D, E, F, G, H, I, J, K, L) -> R {}
}

#[rustfmt::skip]
mod containers {
   use std::{
      ops,
      pin,
      ptr,
      task,
   };

   use super::Dupe;

   impl<A: Dupe> Dupe for Option<A> {}

   impl<T: Dupe, E: Dupe> Dupe for Result<T, E> {}

   impl<A: Dupe> Dupe for ops::Bound<A> {}

   impl<A: Dupe> Dupe for pin::Pin<A> {}

   impl<A: Dupe> Dupe for ptr::NonNull<A> {}

   impl<A: Dupe> Dupe for task::Poll<A> {}

   impl Dupe for () {}
   impl<A: Dupe> Dupe for (A,) {}
   impl<A: Dupe, B: Dupe> Dupe for (A, B) {}
   impl<A: Dupe, B: Dupe, C: Dupe> Dupe for (A, B, C) {}
   impl<A: Dupe, B: Dupe, C: Dupe, D: Dupe> Dupe for (A, B, C, D) {}
   impl<A: Dupe, B: Dupe, C: Dupe, D: Dupe, E: Dupe> Dupe for (A, B, C, D, E) {}
   impl<A: Dupe, B: Dupe, C: Dupe, D: Dupe, E: Dupe, F: Dupe> Dupe for (A, B, C, D, E, F) {}
   impl<A: Dupe, B: Dupe, C: Dupe, D: Dupe, E: Dupe, F: Dupe, G: Dupe> Dupe for (A, B, C, D, E, F, G) {}
   impl<A: Dupe, B: Dupe, C: Dupe, D: Dupe, E: Dupe, F: Dupe, G: Dupe, H: Dupe> Dupe for (A, B, C, D, E, F, G, H) {}
   impl<A: Dupe, B: Dupe, C: Dupe, D: Dupe, E: Dupe, F: Dupe, G: Dupe, H: Dupe, I: Dupe> Dupe for (A, B, C, D, E, F, G, H, I) {}
   impl<A: Dupe, B: Dupe, C: Dupe, D: Dupe, E: Dupe, F: Dupe, G: Dupe, H: Dupe, I: Dupe, J: Dupe> Dupe for (A, B, C, D, E, F, G, H, I, J) {}
   impl<A: Dupe, B: Dupe, C: Dupe, D: Dupe, E: Dupe, F: Dupe, G: Dupe, H: Dupe, I: Dupe, J: Dupe, K: Dupe> Dupe for (A, B, C, D, E, F, G, H, I, J, K) {}
   impl<A: Dupe, B: Dupe, C: Dupe, D: Dupe, E: Dupe, F: Dupe, G: Dupe, H: Dupe, I: Dupe, J: Dupe, K: Dupe, L: Dupe> Dupe for (A, B, C, D, E, F, G, H, I, J, K, L) {}

   impl<A: Dupe, const N: usize> Dupe for [A; N] {}
}

#[cfg(feature = "arcstr")]
mod arcstr {
   use super::Dupe;

   impl Dupe for arcstr::ArcStr {}

   impl Dupe for arcstr::Substr {}
}

#[cfg(feature = "bytes")]
mod bytes {
   use super::Dupe;

   impl Dupe for bytes::Bytes {}

   impl Dupe for bytes::BytesMut {}
}

#[cfg(feature = "cstree")]
mod cstree {
   use cstree::util;

   use super::Dupe;

   impl Dupe for cstree::RawSyntaxKind {}

   impl Dupe for cstree::build::Checkpoint {}

   impl Dupe for cstree::green::GreenNode {}
   impl Dupe for cstree::green::GreenNodeChildren<'_> {}
   impl Dupe for cstree::green::GreenToken {}

   impl Dupe for cstree::interning::TokenKey {}

   impl<T> Dupe for cstree::sync::Arc<T> {}

   impl<S: cstree::Syntax, D> Dupe for cstree::syntax::ResolvedNode<S, D> {}
   impl<S: cstree::Syntax, D> Dupe for cstree::syntax::ResolvedToken<S, D> {}

   impl<S: cstree::Syntax, D> Dupe for cstree::syntax::SyntaxElementChildren<'_, S, D> {}
   impl<S: cstree::Syntax, D> Dupe for cstree::syntax::SyntaxNode<S, D> {}
   impl<S: cstree::Syntax, D> Dupe for cstree::syntax::SyntaxNodeChildren<'_, S, D> {}
   impl<I: Clone, S: cstree::Syntax, D> Dupe for cstree::syntax::SyntaxText<'_, '_, I, S, D> {}
   impl<S: cstree::Syntax, D> Dupe for cstree::syntax::SyntaxToken<S, D> {}

   impl Dupe for cstree::text::TextRange {}
   impl Dupe for cstree::text::TextSize {}

   impl Dupe for cstree::traversal::Direction {}
   impl<T: Dupe> Dupe for cstree::traversal::WalkEvent<T> {}

   impl<N: Dupe, T: Dupe> Dupe for cstree::util::NodeOrToken<N, T> {}
   impl<T: Dupe> Dupe for util::TokenAtOffset<T> {}
}

#[cfg(feature = "rpds")]
mod rpds {
   use std::{
      cmp,
      hash,
   };

   use super::Dupe;

   impl<K: cmp::Eq + hash::Hash, V, P: archery::SharedPointerKind, H: hash::BuildHasher + Clone>
      Dupe for rpds::HashTrieMap<K, V, P, H>
   {
   }

   impl<K: cmp::Eq + hash::Hash, P: archery::SharedPointerKind, H: hash::BuildHasher + Clone> Dupe
      for rpds::HashTrieSet<K, P, H>
   {
   }

   impl<T, P: archery::SharedPointerKind> Dupe for rpds::List<T, P> {}

   impl<T, P: archery::SharedPointerKind> Dupe for rpds::Queue<T, P> {}

   impl<K: cmp::Ord + cmp::Eq + hash::Hash, V, P: archery::SharedPointerKind> Dupe
      for rpds::RedBlackTreeMap<K, V, P>
   {
   }

   impl<T: cmp::Ord + cmp::Eq + hash::Hash, P: archery::SharedPointerKind> Dupe
      for rpds::RedBlackTreeSet<T, P>
   {
   }

   impl<T, P: archery::SharedPointerKind> Dupe for rpds::Stack<T, P> {}

   impl<T, P: archery::SharedPointerKind> Dupe for rpds::Vector<T, P> {}
}
