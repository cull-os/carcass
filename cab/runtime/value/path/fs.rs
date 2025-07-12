use std::{
   iter,
   path::PathBuf,
   sync::Arc,
};

use async_trait::async_trait;
use bytes::Bytes;
use cyn::{
   OptionExt as _,
   Result,
   ResultExt as _,
   bail_tags,
};
use rpds::ListSync as List;
use tokio::fs;
use ust::{
   style::StyledExt as _,
   terminal::tag,
};

use super::{
   Root,
   Subpath,
};

fn to_pathbuf(subpath: &Subpath) -> Result<PathBuf> {
   Ok(if cfg!(target_os = "windows") {
      let mut parts = subpath.iter();

      let drive = parts.by_ref().next().ok_or_tag(&|tags: &mut tag::Tags| {
         tags.write(
            "cannot act on paths without a component to specify the drive on Windows, please \
             specify the drive like so: ",
         );
         tags.write("\\(".yellow());
         tags.write("path.fs");
         tags.write(")/c/path/to/file.txt".yellow());
         tags.write("\nthat expression above is equivalent to ");
         tags.write("C:\\path\\to\\file.txt".yellow());
      })?;

      if !drive.chars().all(|c| c.is_ascii_lowercase()) {
         bail_tags!(&|tags: &mut tag::Tags| {
            tags.write("drive components must be lowercase, like so: ");
            tags.write("\\(".yellow());
            tags.write("path.fs");
            tags.write(")/".yellow());
            tags.write("c".red());
            tags.write("C".green());
            tags.write("/path/to/file.txt".yellow());
            tags.write("\nwhy? WSL compatibility");
         });
      }

      // Make it uppercase (only on windows) just becase.
      // The kernel doesn't care, but it's cooler to read
      // C:\ rather than c:\.
      let drive = drive.to_ascii_uppercase();

      iter::once(&*format!("{drive}:\\"))
         .chain(parts.map(|arc| &**arc))
         .collect::<PathBuf>()
   } else {
      iter::once("/")
         .chain(subpath.iter().map(|arc| &**arc))
         .collect::<PathBuf>()
   })
}

#[must_use]
pub fn fs() -> impl Root {
   Fs
}

struct Fs;

#[async_trait]
impl Root for Fs {
   fn type_(&self) -> &'static str {
      "fs"
   }

   async fn list(self: Arc<Self>, subpath: &Subpath) -> Result<List<Subpath>> {
      let mut contents = Vec::new();

      let path = to_pathbuf(subpath)?;

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

         contents.push(subpath.push_front(Arc::from(name)));
      }

      contents.sort_unstable();

      Ok(rpds::List::from_iter(contents))
   }

   async fn read(self: Arc<Self>, subpath: &Subpath) -> Result<Bytes> {
      let path = to_pathbuf(subpath)?;

      let content = fs::read(&path)
         .await
         .chain_err_with(|| format!("failed to read '{path}'", path = path.display()))?;

      Ok(Bytes::from(content))
   }

   async fn is_writeable(&self) -> bool {
      true
   }

   async fn write(self: Arc<Self>, subpath: &Subpath, content: Bytes) -> Result<()> {
      let path = to_pathbuf(subpath)?;

      fs::write(&path, &content)
         .await
         .chain_err_with(|| format!("failed to write to '{path}'", path = path.display()))
   }
}
