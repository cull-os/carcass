use core::fmt;
use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use cab_why::Result;

use crate::{
   Entry,
   Leaf,
};

/// Creates a leaf from the given content.
pub fn blob(content: impl Into<Bytes>) -> impl Leaf {
   Blob {
      content: content.into(),
   }
}

struct Blob {
   content: Bytes,
}

impl fmt::Display for Blob {
   fn fmt(&self, writer: &mut fmt::Formatter<'_>) -> fmt::Result {
      write!(writer, "blob")
   }
}

#[async_trait]
impl Entry for Blob {
   async fn as_leaf(self: Arc<Self>) -> Option<Arc<dyn Leaf>> {
      Some(self)
   }
}

#[async_trait]
impl Leaf for Blob {
   async fn read(self: Arc<Self>) -> Result<Bytes> {
      Ok(self.content.clone())
   }
}
