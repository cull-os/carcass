use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use cab_report::{
   Error,
   Result,
   bail,
   error,
};

#[async_trait]
pub trait Root: Send + Sync + 'static {
   async fn list(self: Arc<Self>, content: &str) -> Result<Arc<[Path]>> {
      bail!("TODO list '{content:?}' error")
   }

   async fn get(self: Arc<Self>, content: &str) -> Result<Path> {
      let list = self.list(content).await?;

      list
         .iter()
         .find(|entry| &*entry.content == content)
         .cloned()
         .ok_or_else(|| error!("TODO get '{content:?}' error"))
   }

   async fn read(self: Arc<Self>, content: &str) -> Result<Bytes> {
      bail!("TODO read '{content:?}' error")
   }
}

#[derive(Clone)]
pub struct Path {
   root:    Option<Arc<dyn Root>>,
   content: Arc<str>,
}

impl TryInto<Arc<str>> for Path {
   type Error = Error;

   fn try_into(self) -> Result<Arc<str>> {
      if self.root.is_some() {
         bail!("TODO");
      }

      Ok(self.content)
   }
}

impl Path {
   #[must_use]
   pub fn new(root: Arc<dyn Root>, content: Arc<str>) -> Self {
      Self {
         root: Some(root),
         content,
      }
   }

   #[must_use]
   pub fn rootless(content: Arc<str>) -> Self {
      Self {
         root: None,
         content,
      }
   }
}
