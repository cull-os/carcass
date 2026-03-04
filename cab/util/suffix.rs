//! Extension traits that provide concise suffix-style helper methods.

use std::sync;

/// Extension trait for wrapping values in [`sync::Arc`].
///
/// This trait is implemented for all `T`, which makes `.arc()` available on
/// any sized value.
pub trait Arc {
   /// Wraps `self` in a newly allocated [`sync::Arc`].
   fn arc(self) -> sync::Arc<Self>
   where
      Self: Sized,
   {
      sync::Arc::new(self)
   }
}

impl<T> Arc for T {}
