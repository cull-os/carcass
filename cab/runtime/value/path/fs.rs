use std::{
   path::PathBuf,
   sync::Arc,
};

use async_trait::async_trait;
use bytes::Bytes;
use cab_error::{
   OptionExt as _,
   Result,
   ResultExt as _,
};
use rpds::ListSync as List;
use tokio::fs;

use super::{
   Root,
   Subpath,
};
use crate::Value;

#[must_use]
pub fn fs(config: Value, path: Value) -> impl Root {
   Fs { config, path }
}

struct Fs {
   config: Value,
   path:   Value,
}

#[async_trait]
impl Root for Fs {
   fn type_(&self) -> &'static str {
      "fs"
   }

   fn config(&self) -> Option<&Value> {
      Some(&self.config)
   }

   fn path(&self) -> Option<&Value> {
      Some(&self.path)
   }

   async fn list(self: Arc<Self>, subpath: &Subpath) -> Result<List<Subpath>> {
      let mut contents = List::new_sync();

      let path = self.to_pathbuf(subpath);

      let mut read = fs::read_dir(&path)
         .await
         .chain_err_with(|| format!("failed to read dir '{path}'", path = path.display()))?;

      while let Some(entry) = read
         .next_entry()
         .await
         .chain_err_with(|| format!("failed to read entry of '{path}'", path = path.display()))?
      {
         let name = entry.file_name();
         let name = name.to_str().ok_or_chain_with(|| {
            format!(
               "entry with name similar to '{name}' has a name that is not valid UTF-8",
               name = name.display()
            )
         })?;

         contents = contents.push_front(subpath.push_front(name.into()));
      }

      Ok(contents)
   }

   async fn read(self: Arc<Self>, subpath: &Subpath) -> Result<Bytes> {
      let path = self.to_pathbuf(subpath);

      let content = fs::read(&path)
         .await
         .chain_err_with(|| format!("failed to read '{path}'", path = path.display()))?;

      Ok(Bytes::from(content))
   }

   async fn is_writeable(&self) -> bool {
      true
   }

   async fn write(self: Arc<Self>, subpath: &Subpath, content: Bytes) -> Result<()> {
      let path = self.to_pathbuf(subpath);

      fs::write(&path, &content)
         .await
         .chain_err_with(|| format!("failed to write to '{path}'", path = path.display()))
   }
}

impl Fs {
   fn to_pathbuf(&self, subpath: &Subpath) -> PathBuf {
      let Value::Path(ref path) = self.path else {
         unreachable!()
      };

      assert!(path.root.is_none());

      path
         .subpath
         .iter()
         .map(|arc| &**arc)
         .chain(subpath.iter().map(|arc| &**arc))
         .collect::<PathBuf>()
   }
}
