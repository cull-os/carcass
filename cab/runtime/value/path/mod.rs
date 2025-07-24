use std::{
   iter,
   sync::Arc,
};

use async_once_cell::OnceCell;
use async_trait::async_trait;
use bytes::Bytes;
use cyn::{
   OptionExt as _,
   Result,
   ResultExt as _,
   bail,
};
use dashmap::DashMap;
use dup::{
   Dupe,
   IteratorDupedExt as _,
};
use rpds::ListSync as List;
use ust::{
   style::StyledExt as _,
   terminal::tag,
};

use super::Value;

mod blob;
pub use blob::blob;

mod fs;
pub use fs::fs;

mod standard;
pub use standard::standard;

pub const SEPARATOR: char = '/';

pub type Part = Arc<str>;

pub type Subpath = List<Part>;

#[async_trait]
pub trait Root: Send + Sync + 'static {
   fn type_(&self) -> &'static str;

   fn config(&self) -> Option<&Value> {
      None
   }

   async fn list(self: Arc<Self>, subpath: &Subpath) -> Result<List<Subpath>> {
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

#[derive(Clone, Dupe)]
pub struct Path {
   root:    Option<Arc<dyn Root>>,
   subpath: Subpath,

   read_cache: Arc<DashMap<Subpath, OnceCell<Result<Bytes>>>>,
   list_cache: Arc<DashMap<Subpath, OnceCell<Result<List<Subpath>>>>>,
}

impl tag::DisplayTags for Path {
   fn display_tags<'a>(&'a self, tags: &mut tag::Tags<'a>) {
      if self.root.is_none() && self.subpath.is_empty() {
         tags.write("<empty-path>".bright_black());
         return;
      }

      if let Some(ref root) = self.root {
         let type_ = root.type_();
         let config = root.config();

         tags.write("\\(".yellow());
         tags.write("path");
         tags.write(".".magenta());
         tags.write(type_.yellow());

         if let Some(config) = config {
            tags.write(" ".yellow());
            config.display_tags(tags);
         }

         tags.write(")".yellow());
      }

      for part in &self.subpath {
         tags.write(const_str::concat!(SEPARATOR).yellow());
         tags.write((**part).yellow());
      }
   }
}

impl Path {
   #[must_use]
   pub fn new(root: Arc<dyn Root>, subpath: Subpath) -> Self {
      Self {
         root: Some(root),
         subpath,

         read_cache: Arc::new(DashMap::new()),
         list_cache: Arc::new(DashMap::new()),
      }
   }

   #[must_use]
   pub fn rootless(subpath: Subpath) -> Self {
      Self {
         root: None,
         subpath,

         read_cache: Arc::new(DashMap::new()),
         list_cache: Arc::new(DashMap::new()),
      }
   }
}

impl Path {
   #[must_use]
   pub fn get(&self, part: Part) -> Self {
      Self {
         root:    self.root.dupe(),
         subpath: self
            .subpath
            .iter()
            .duped()
            .chain(iter::once(part))
            .collect(),

         read_cache: self.read_cache.dupe(),
         list_cache: self.list_cache.dupe(),
      }
   }

   pub async fn list(&self) -> Result<List<Subpath>> {
      let cache = self.list_cache.get(&self.subpath).unwrap_or_else(|| {
         self.list_cache.entry(self.subpath.dupe()).or_default();
         self.list_cache.get(&self.subpath).unwrap()
      });

      cache
         .get_or_init(async {
            let root = self.root.dupe().ok_or_tag(&|tags: &mut tag::Tags| {
               tags.write("tried to list rootless path ");
               tags.extend(self);
            })?;

            root
               .list(&self.subpath)
               .await
               .tag_err(&|tags: &mut tag::Tags| {
                  tags.write("failed to read ");
                  tags.extend(self);
               })
         })
         .await
         .dupe()
   }

   pub async fn read(&self) -> Result<Bytes> {
      let cache = self.read_cache.get(&self.subpath).unwrap_or_else(|| {
         self.read_cache.entry(self.subpath.dupe()).or_default();
         self.read_cache.get(&self.subpath).unwrap()
      });

      cache
         .get_or_init(async {
            let root = self.root.dupe().ok_or_tag(&|tags: &mut tag::Tags| {
               tags.write("tried to read rootless path ");
               tags.extend(self);
            })?;

            root
               .read(&self.subpath)
               .await
               .tag_err(&|tags: &mut tag::Tags| {
                  tags.write("failed to read ");
                  tags.extend(self);
               })
         })
         .await
         .dupe()
   }

   pub async fn write(&self, content: Bytes) -> Result<()> {
      let root = self.root.dupe().ok_or_tag(&|tags: &mut tag::Tags| {
         tags.write("tried to write to rootless path ");
         tags.extend(self);
      })?;

      root
         .write(&self.subpath, content)
         .await
         .tag_err(&|tags: &mut tag::Tags| {
            tags.write("failed to write to ");
            tags.extend(self);
         })
   }
}
