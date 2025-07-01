//! Cheap clone trait for distinguishing expensive clones from things that
//! should have been Copy.

use std::iter;

pub use dup_macros::Dupe;

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

mod pointers {
   use std::{
      cell,
      mem,
      rc,
      sync,
   };

   use super::Dupe;

   impl<A: ?Sized> Dupe for sync::Arc<A> {}

   impl<A: ?Sized> Dupe for sync::Weak<A> {}

   impl<A: ?Sized> Dupe for rc::Rc<A> {}

   impl<A: ?Sized> Dupe for rc::Weak<A> {}

   impl<A: Copy> Dupe for cell::Cell<A> {}

   impl<A: Dupe> Dupe for mem::ManuallyDrop<A> {}
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

   impl<A: Dupe, const N: usize> Dupe for [A; N] {}
}

#[cfg(feature = "bytes")]
mod bytes {
   use super::Dupe;

   impl Dupe for bytes::Bytes {}

   impl Dupe for bytes::BytesMut {}
}

#[cfg(feature = "rpds")]
mod rpds {
   use std::{
      cmp,
      hash,
   };

   use super::Dupe;

   impl<K: cmp::Eq + hash::Hash, V> Dupe for rpds::HashTrieMap<K, V> {}
   impl<K: cmp::Eq + hash::Hash, V> Dupe for rpds::HashTrieMapSync<K, V> {}

   impl<T: cmp::Eq + hash::Hash> Dupe for rpds::HashTrieSet<T> {}
   impl<T: cmp::Eq + hash::Hash> Dupe for rpds::HashTrieSetSync<T> {}

   impl<T> Dupe for rpds::List<T> {}
   impl<T> Dupe for rpds::ListSync<T> {}

   impl<T> Dupe for rpds::Queue<T> {}
   impl<T> Dupe for rpds::QueueSync<T> {}

   impl<K: cmp::Ord + cmp::Eq + hash::Hash, V> Dupe for rpds::RedBlackTreeMap<K, V> {}
   impl<K: cmp::Ord + cmp::Eq + hash::Hash, V> Dupe for rpds::RedBlackTreeMapSync<K, V> {}

   impl<T: cmp::Ord + cmp::Eq + hash::Hash> Dupe for rpds::RedBlackTreeSet<T> {}
   impl<T: cmp::Ord + cmp::Eq + hash::Hash> Dupe for rpds::RedBlackTreeSetSync<T> {}

   impl<T> Dupe for rpds::Stack<T> {}
   impl<T> Dupe for rpds::StackSync<T> {}

   impl<T> Dupe for rpds::Vector<T> {}
   impl<T> Dupe for rpds::VectorSync<T> {}
}
