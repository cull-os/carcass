use std::{
    fmt,
    path::{
        Path,
        PathBuf,
    },
    sync::Arc,
};

use async_once_cell::OnceCell;
use async_trait::async_trait;
use bytes::Bytes;
use cab_why::{
    Contextful as _,
    bail,
};
use tokio::fs;

use crate::{
    Collection,
    CollectionPeek,
    Entry,
    Leaf,
    Result,
    display,
};

pub fn fs(path: PathBuf) -> impl Leaf + CollectionPeek {
    FsEntry {
        location: FsEntryLocation::Root { path },

        content: OnceCell::new(),
    }
}

#[derive(Clone)]
enum FsEntryContent {
    Leaf(Bytes),
    CollectionPeek(Arc<[Arc<dyn Entry>]>),
}

enum FsEntryLocation {
    Root { path: PathBuf },
    Child { parent: Arc<FsEntry>, name: String },
}

struct FsEntry {
    location: FsEntryLocation,

    content: OnceCell<Result<FsEntryContent>>,
}

impl fmt::Display for FsEntry {
    fn fmt(&self, writer: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.location {
            FsEntryLocation::Root { path } => write!(writer, "fs::{path}", path = path.to_str().ok_or(fmt::Error)?),
            FsEntryLocation::Child { name, .. } => write!(writer, "{name}"),
        }
    }
}

#[async_trait]
impl Entry for FsEntry {
    fn name(&self) -> Option<&str> {
        match &self.location {
            FsEntryLocation::Root { .. } => None,
            FsEntryLocation::Child { name, .. } => Some(name),
        }
    }

    fn parent(&self) -> Option<Arc<dyn Collection>> {
        match &self.location {
            FsEntryLocation::Root { .. } => None,
            FsEntryLocation::Child { parent, .. } => Some(parent.clone()),
        }
    }

    // TODO: Maybe not do this and check the content?
    async fn as_leaf(self: Arc<Self>) -> Option<Arc<dyn Leaf>> {
        Some(self)
    }

    async fn as_collection(self: Arc<Self>) -> Option<Arc<dyn Collection>> {
        Some(self)
    }

    async fn as_collection_peek(self: Arc<Self>) -> Option<Arc<dyn CollectionPeek>> {
        Some(self)
    }
}

#[async_trait]
impl Leaf for FsEntry {
    async fn read(self: Arc<Self>) -> Result<Bytes> {
        match self.content().await? {
            FsEntryContent::Leaf(bytes) => Ok(bytes),

            FsEntryContent::CollectionPeek(_) => {
                bail!("failed to read {this} as it is a directory", this = display!(self))
            },
        }
    }
}

#[async_trait]
impl Collection for FsEntry {
    async fn entry(self: Arc<Self>, name: &str) -> Result<Option<Arc<dyn Entry>>> {
        Ok(self
            .list()
            .await?
            .iter()
            .find(|entry| entry.name() == Some(name))
            .map(Arc::clone))
    }
}

#[async_trait]
impl CollectionPeek for FsEntry {
    async fn list(self: Arc<Self>) -> Result<Arc<[Arc<dyn Entry>]>> {
        match self.content().await? {
            FsEntryContent::CollectionPeek(entries) => Ok(entries),

            FsEntryContent::Leaf(_) => {
                bail!("failed to list {this} as it is a file", this = display!(self))
            },
        }
    }
}

impl FsEntry {
    fn path(&self) -> PathBuf {
        let mut this = self;
        let mut parts = Vec::new();

        loop {
            match &this.location {
                FsEntryLocation::Root { path } => break parts.push(path.as_path()),

                FsEntryLocation::Child { parent, name, .. } => {
                    this = parent;

                    parts.push(Path::new(name.as_str()));
                },
            }
        }

        PathBuf::from_iter(parts.into_iter().rev())
    }

    async fn content(self: &Arc<Self>) -> Result<FsEntryContent> {
        self.content.get_or_init(self.content_eager()).await.clone()
    }

    async fn content_eager(self: &Arc<Self>) -> Result<FsEntryContent> {
        let path = self.path();

        let metadata = fs::metadata(&path)
            .await
            .with_context(|| format!("failed to get metadata of '{path}'", path = path.to_string_lossy()))?;

        if metadata.is_file() || metadata.is_symlink() {
            return self.content_file_eager(&path).await;
        }

        if metadata.is_dir() {
            return self.content_dir_eager(&path).await;
        }

        bail!("unsupported type of entry at '{path}'", path = path.to_string_lossy());
    }

    async fn content_file_eager(self: &Arc<Self>, path: &Path) -> Result<FsEntryContent> {
        let bytes = fs::read(path)
            .await
            .with_context(|| format!("failed to read '{path}'", path = path.to_string_lossy()))?;

        Ok(FsEntryContent::Leaf(Bytes::from(bytes)))
    }

    async fn content_dir_eager(self: &Arc<Self>, path: &Path) -> Result<FsEntryContent> {
        let mut read_dir = fs::read_dir(path)
            .await
            .with_context(|| format!("failed to list '{path}'", path = path.to_string_lossy()))?;

        let mut entries = Vec::<Arc<dyn Entry>>::new();

        while let Some(entry) = read_dir
            .next_entry()
            .await
            .with_context(|| format!("failed to read entry under '{path}'", path = path.to_string_lossy()))?
        {
            let name = entry.file_name();

            entries.push(Arc::new(FsEntry {
                location: FsEntryLocation::Child {
                    parent: self.clone(),
                    name: name
                        .to_str()
                        .with_context(|| {
                            format!(
                                "failed to convert name of '{name}' under '{path}' to valid UTF-8",
                                name = name.to_string_lossy(),
                                path = path.to_string_lossy(),
                            )
                        })?
                        .to_owned(),
                },

                content: OnceCell::new(),
            }))
        }

        Ok(FsEntryContent::CollectionPeek(entries.into()))
    }
}
