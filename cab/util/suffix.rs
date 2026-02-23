use std::sync;

pub trait Arc {
   fn arc(self) -> sync::Arc<Self>
   where
      Self: Sized,
   {
      sync::Arc::new(self)
   }
}

impl<T> Arc for T {}
