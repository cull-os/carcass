use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use cab_error::{
   Result,
   ResultExt as _,
   bail,
};
use tokio::io::{
   self,
   AsyncReadExt as _,
   AsyncWriteExt as _,
};

use super::{
   Root,
   Subpath,
};
use crate::Value;

#[must_use]
pub fn standard() -> impl Root {
   Standard
}

struct Standard;

#[async_trait]
impl Root for Standard {
   fn type_(&self) -> &'static str {
      "standard"
   }

   fn config(&self) -> Option<&Value> {
      None
   }

   fn path(&self) -> Option<&Value> {
      None
   }

   async fn read(self: Arc<Self>, subpath: &Subpath) -> Result<Bytes> {
      if !subpath.is_empty() {
         bail!("standard only contains a single leaf");
      }

      let mut buffer = Vec::new();

      io::stdin()
         .read_to_end(&mut buffer)
         .await
         .chain_err("failed to read from standard in")?;

      Ok(Bytes::from(buffer))
   }

   async fn is_writeable(&self) -> bool {
      true
   }

   async fn write(self: Arc<Self>, subpath: &Subpath, content: Bytes) -> Result<()> {
      if !subpath.is_empty() {
         bail!("standard only contains a single leaf");
      }

      io::stdout()
         .write_all(&content)
         .await
         .chain_err("failed to write to standard out")
   }
}
