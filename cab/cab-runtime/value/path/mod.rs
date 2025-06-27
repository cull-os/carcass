use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use cab_error::{
   Contextful as _,
   Result,
   bail,
};
use ust::{
   style::StyledExt as _,
   terminal::tag,
};

use super::Value;

mod blob;
pub use blob::blob;

mod standard;
pub use standard::standard;
// mod fs;
// mod stdin;

pub const SEPARATOR: char = '/';

pub type Subpath = Arc<[Arc<str>]>;

#[async_trait]
pub trait Root: Send + Sync + 'static {
   fn type_(&self) -> &'static str;

   fn config(&self) -> Option<&Value>;

   fn path(&self) -> Option<&Value>;

   async fn list(self: Arc<Self>, subpath: &Subpath) -> Result<Arc<[Path]>> {
      let _ = subpath;

      bail!("root does not support listing");
   }

   async fn read(self: Arc<Self>, subpath: &Subpath) -> Result<Bytes>;

   async fn is_writeable(&self) -> bool {
      false
   }

   async fn write(self: Arc<Self>, subpath: &Subpath, content: Bytes) -> Result<()> {
      let _ = (subpath, content);

      bail!("root does not support writing");
   }
}

#[derive(Clone)]
pub struct Path {
   root:    Option<Arc<dyn Root>>,
   subpath: Subpath,
}

impl tag::DisplayTags for Path {
   fn display_tags<'a>(&'a self, tags: &mut tag::Tags<'a>) {
      if let Some(ref root) = self.root {
         let type_ = root.type_();
         let config = root.config();
         let path = root.path();

         tags.write("<".yellow());
         tags.write(type_.yellow());

         match config {
            None => {
               if let Some(path) = path {
                  tags.write("::".yellow());
                  path.display_tags(tags);
               }
            },

            Some(config) => {
               tags.write(":".yellow());
               config.display_tags(tags);

               if let Some(path) = path {
                  tags.write(":".yellow());
                  path.display_tags(tags);
               }
            },
         }

         tags.write(">");
      }

      if self.subpath.is_empty() {
         tags.write("<empty-path>".bright_black());
      } else {
         for part in &*self.subpath {
            tags.write(const_str::concat!(SEPARATOR).yellow());
            tags.write((**part).yellow());
         }
      }
   }
}

impl From<Path> for Value {
   fn from(path: Path) -> Self {
      Value::Path(path)
   }
}

impl Path {
   #[must_use]
   pub fn new(root: Arc<dyn Root>, subpath: Subpath) -> Self {
      Self {
         root: Some(root),
         subpath,
      }
   }

   #[must_use]
   pub fn rootless(subpath: Subpath) -> Self {
      Self {
         root: None,
         subpath,
      }
   }
}

impl Path {
   #[must_use]
   pub fn get(&self, subpath: Arc<str>) -> Self {
      Self {
         root:    self.root.clone(),
         subpath: self.subpath.iter().cloned().chain([subpath]).collect(),
      }
   }

   pub async fn list(&self) -> Result<Arc<[Path]>> {
      let root = self
         .root
         .clone()
         .context("tried to list rootless path 'TODO'")?;

      root
         .list(&self.subpath)
         .await
         .context("failed to read TODO")
   }

   pub async fn read(&self) -> Result<Bytes> {
      let root = self.root.clone().context("tried to read rootless path")?;

      root
         .read(&self.subpath)
         .await
         .context("failed to read TODO")
   }

   pub async fn write(&self, content: Bytes) -> Result<()> {
      let root = self
         .root
         .clone()
         .context("tried to write to rootless path 'TODO'")?;

      root
         .write(&self.subpath, content)
         .await
         .context("failed to write to TODO")
   }
}
