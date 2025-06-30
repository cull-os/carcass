use std::{
   path::PathBuf,
   sync::Arc,
};

use async_once_cell::OnceCell;
use async_trait::async_trait;
use bytes::Bytes;
use cab_error::{
   Contextful as _,
   Result,
};
use dashmap::DashMap;
use rpds::ListSync as List;
use rustc_hash::FxBuildHasher;
use tokio::fs;

use super::{
   Root,
   Subpath,
};
use crate::Value;

/// Creates an entry from a given fs path.
#[must_use]
pub fn fs(config: Value, path: Value) -> impl Root {
   Fs {
      config,
      path,

      entries: DashMap::with_hasher(FxBuildHasher),
      contents: DashMap::with_hasher(FxBuildHasher),
   }
}

struct Fs {
   config: Value,
   path:   Value,

   entries:  DashMap<Subpath, OnceCell<Result<List<Subpath>>>, FxBuildHasher>,
   contents: DashMap<Subpath, OnceCell<Result<Bytes>>, FxBuildHasher>,
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
      self
         .entries
         .entry(subpath.clone())
         .or_default()
         .get_or_init(async {
            let mut contents = Vec::new();

            let path = self.to_pathbuf(subpath);

            let mut read = fs::read_dir(&path)
               .await
               .with_context(|| format!("failed to read dir '{path}'", path = path.display()))?;

            while let Some(entry) = read.next_entry().await.with_context(|| {
               format!("failed to read entry of '{path}'", path = path.display())
            })? {
               let name = entry.file_name();
               let name = name.to_str().with_context(|| {
                  format!(
                     "entry with name similar to '{name}' contains invalid UTF-8",
                     name = name.display()
                  )
               })?;

               contents.push(subpath.push_front(name.into()));
            }

            todo!()
         })
         .await
         .clone()
   }

   async fn read(self: Arc<Self>, subpath: &Subpath) -> Result<Bytes> {
      self
         .contents
         .entry(subpath.clone())
         .or_default()
         .get_or_init(async {
            let path = self.to_pathbuf(subpath);

            let content = fs::read(&path)
               .await
               .with_context(|| format!("failed to read '{path}'", path = path.display()))?;

            Ok(Bytes::from(content))
         })
         .await
         .clone()
   }

   async fn is_writeable(&self) -> bool {
      true
   }

   async fn write(self: Arc<Self>, _subpath: &Subpath, _content: Bytes) -> Result<()> {
      todo!()
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
