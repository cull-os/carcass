use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use cyn::{
   Result,
   bail,
};

use super::{
   Root,
   Subpath,
};
use crate::Value;

#[must_use]
pub fn blob(config: Value) -> impl Root {
   Blob { config }
}

struct Blob {
   config: Value,
}

#[async_trait]
impl Root for Blob {
   fn type_(&self) -> &'static str {
      "blob"
   }

   fn config(&self) -> Option<&Value> {
      Some(&self.config)
   }

   async fn read(self: Arc<Self>, subpath: &Subpath) -> Result<Bytes> {
      if !subpath.is_empty() {
         bail!("blob only contains a single leaf");
      }

      let Value::String(ref string) = self.config else {
         unreachable!()
      };

      Ok(Bytes::copy_from_slice(string.as_bytes()))
   }
}
