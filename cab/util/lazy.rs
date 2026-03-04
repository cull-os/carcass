//! Convenience macros for working with [`std::cell::LazyCell`].

/// Type shorthand for a [`std::cell::LazyCell`] value.
///
/// # Example
///
/// ```rs
/// # use cab_util::Lazy;
/// fn read_from(mut string: Lazy!(String)) {
///    // ... do some work ...
///
///    println!("{string}", string = &*string);
/// }
/// ```
#[macro_export]
macro_rules! Lazy {
   ($type:ty) => {
      std::cell::LazyCell<$type, impl FnOnce() -> $type>
   };
}

/// Creates a [`std::cell::LazyCell`].
///
/// # Example
///
/// ```rs
/// # use cab_util::{force, lazy};
/// let mut answer = lazy!(40 + 2);
/// assert_eq!(*force!(answer), 42);
/// ```
#[macro_export]
macro_rules! lazy {
   ($expression:expr) => {
      std::cell::LazyCell::new(|| $expression)
   };
}

/// Forces a lazy value and returns `&mut T`.
#[macro_export]
macro_rules! force {
   ($expression:expr) => {
      std::cell::LazyCell::force_mut(&mut $expression)
   };
}

/// Forces a lazy value through a mutable reference and returns `&mut
/// T`.
///
/// # Example
///
/// ```rs
/// # use cab_util::{force_ref, lazy};
/// let mut answer = lazy!(40 + 2);
/// let answer_ref = &mut answer;
/// assert_eq!(*force_ref!(answer_ref), 42);
/// ```
#[macro_export]
macro_rules! force_ref {
   ($expression:expr) => {
      std::cell::LazyCell::force_mut($expression)
   };
}

/// Consumes a lazy value and returns its initialized value, if present.
///
/// Returns `None` when the [`std::cell::LazyCell`] was never forced.
///
/// # Example
///
/// ```rs
/// # use cab_util::{force, lazy, read};
/// let mut answer = lazy!(40 + 2);
/// assert_eq!(*force!(answer), 42);
/// assert_eq!(read!(answer), Some(42));
/// ```
#[macro_export]
macro_rules! read {
   ($expression:expr) => {
      std::cell::LazyCell::into_inner($expression).ok()
   };
}

/// Returns whether a lazy value has been forced.
///
/// # Example
///
/// ```rs
/// # use std::cell::LazyCell;
/// # use cab_util::{lazy, ready};
/// let answer = lazy!(40 + 2);
/// assert!(!ready!(answer));
/// let _ = LazyCell::force(&answer);
/// assert!(ready!(answer));
/// ```
#[macro_export]
macro_rules! ready {
   ($expression:expr) => {
      std::cell::LazyCell::get(&$expression).is_some()
   };
}
