#[macro_export]
macro_rules! Lazy {
   ($type:ty) => {
      std::cell::LazyCell<$type, impl FnOnce() -> $type>
   };
}

#[macro_export]
macro_rules! lazy {
   ($expression:expr) => {
      std::cell::LazyCell::new(|| $expression)
   };
}

#[macro_export]
macro_rules! force {
   ($ident:ident) => {
      std::cell::LazyCell::force_mut(&mut $ident)
   };
}

#[macro_export]
macro_rules! force_ref {
   ($ident:ident) => {
      std::cell::LazyCell::force_mut($ident)
   };
}

#[macro_export]
macro_rules! read {
   ($expression:expr) => {
      std::cell::LazyCell::into_inner($expression).ok()
   };
}

#[macro_export]
macro_rules! ready {
   ($expression:expr) => {
      std::cell::LazyCell::get(&$expression).is_some()
   };
}
