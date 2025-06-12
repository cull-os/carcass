use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use cab_error::{
   Error,
   Result,
   bail,
};
use ust::{
   style::StyledExt as _,
   terminal::tag,
};

use super::Value;

#[async_trait]
pub trait Root: Send + Sync + 'static {
   fn type_(&self) -> &Arc<str>;

   fn config(&self) -> &Value;

   fn path(&self) -> &Value;

   async fn list(self: Arc<Self>, content: &str) -> Result<Arc<[Path]>> {
      bail!("TODO list '{content:?}' error")
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
                  Value::Path(ref path) if path.content.is_empty() => {},

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
                  Value::Path(ref path) if path.content.is_empty() => {},

                  ref path => {
                     tags.write(":".yellow());
                     path.display_tags(tags);
                  },
               }
            },
         }

         tags.write(">");
      }

      if self.content.is_empty() {
         tags.write("<empty-path>".red());
      } else {
         tags.write((*self.content).yellow());
      }
   }
}

impl From<Path> for Value {
   fn from(path: Path) -> Self {
      Value::Path(path)
   }
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

impl Path {
   #[must_use]
   pub fn get(&self, content: &str) -> Self {
      let mut content_ = String::with_capacity(self.content.len() + content.len());

      content_.push_str(&self.content);
      content_.push_str(content);

      Self {
         root:    self.root.clone(),
         content: content_.into(),
      }
   }

   pub async fn read(&self) -> Result<Bytes> {
      let Some(root) = self.root.clone() else {
         bail!("tried to read rootless path");
      };

      root.read(&self.content).await
   }

   pub async fn list(&self) -> Result<Arc<[Path]>> {
      let Some(root) = self.root.clone() else {
         bail!("tried to list rootless path");
      };

      root.list(&self.content).await
   }
}
