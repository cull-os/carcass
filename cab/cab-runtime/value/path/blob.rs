use std::sync::{
   Arc,
   OnceLock,
};

use async_trait::async_trait;
use bytes::Bytes;
use cab_error::{
   Result,
   bail,
};
use dup::Dupe as _;

use super::{
   Root,
   Subpath,
};
use crate::Value;

#[must_use]
pub fn blob(config: Value) -> impl Root {
   Blob {
      config,

      content: OnceLock::new(),
   }
}

struct Blob {
   config: Value,

   content: OnceLock<Bytes>,
}

#[async_trait]
impl Root for Blob {
   fn type_(&self) -> &'static str {
      "blob"
   }

   fn config(&self) -> Option<&Value> {
      Some(&self.config)
   }

   fn path(&self) -> Option<&Value> {
      None
   }

   async fn read(self: Arc<Self>, subpath: &Subpath) -> Result<Bytes> {
      if !subpath.is_empty() {
         bail!("blob only contains a single leaf");
      }

      Ok(self
         .content
         .get_or_init(|| {
            let Value::String(ref string) = self.config else {
               unreachable!()
            };

            Bytes::copy_from_slice(string.as_bytes())
         })
         .dupe())
   }
}
