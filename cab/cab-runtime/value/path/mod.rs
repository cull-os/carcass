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
// mod fs;
// mod stdin;

#[async_trait]
pub trait Root: Send + Sync + 'static {
   fn new(config: Value, path: Value) -> Result<Self>
   where
      Self: Sized;

   fn type_(&self) -> &Arc<str>;

   fn config(&self) -> &Value;

   fn path(&self) -> &Value;

   async fn list(self: Arc<Self>, subpath: &str) -> Result<Arc<[Path]>> {
      let _ = subpath;

      bail!("root does not support listing");
   }

   async fn read(self: Arc<Self>, subpath: &str) -> Result<Bytes> {
      let _ = subpath;

      bail!("root does not support reading");
   }

   async fn is_mutable(&self) -> bool {
      false
   }

   async fn write(self: Arc<Self>, subpath: &str, content: Bytes) -> Result<()> {
      let _ = (subpath, content);

      bail!("root does not support writing");
   }
}

#[derive(Clone)]
pub struct Path {
   root:    Option<Arc<dyn Root>>,
   subpath: Arc<str>,
}

impl tag::DisplayTags for Path {
   fn display_tags<'a>(&'a self, tags: &mut tag::Tags<'a>) {
      if let Some(ref root) = self.root {
         let type_ = root.type_();
         let config = root.config();
         let path = root.path();

         tags.write("<".yellow());
         tags.write((**type_).yellow());

         match *config {
            Value::Attributes(ref attributes) if attributes.is_empty() => {
               match *path {
                  Value::Path(ref path) if path.subpath.is_empty() => {},

                  ref path => {
                     tags.write("::".yellow());
                     path.display_tags(tags);
                  },
               }
            },

            ref config => {
               tags.write(":".yellow());
               config.display_tags(tags);

               match *path {
                  Value::Path(ref path) if path.subpath.is_empty() => {},

                  ref path => {
                     tags.write(":".yellow());
                     path.display_tags(tags);
                  },
               }
            },
         }

         tags.write(">");
      }

      if self.subpath.is_empty() {
         tags.write("<empty-path>".bright_black());
      } else {
         tags.write((*self.subpath).yellow());
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
   pub fn new(root: Arc<dyn Root>, subpath: Arc<str>) -> Self {
      Self {
         root: Some(root),
         subpath,
      }
   }

   #[must_use]
   pub fn rootless(subpath: Arc<str>) -> Self {
      Self {
         root: None,
         subpath,
      }
   }
}

impl Path {
   #[must_use]
   pub fn get(&self, subpath: &str) -> Self {
      let mut subpath_ = String::with_capacity(self.subpath.len() + subpath.len());

      subpath_.push_str(&self.subpath);
      subpath_.push_str(subpath);

      Self {
         root:    self.root.clone(),
         subpath: subpath_.into(),
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
