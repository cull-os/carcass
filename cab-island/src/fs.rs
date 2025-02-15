use std::{
    fmt,
    path::{
        Path,
        PathBuf,
    },
    sync::Arc,
};

use async_trait::async_trait;
use bytes::Bytes;
use cab_error::{
    Contextful,
    bail,
};
use futures::{
    FutureExt,
    future::BoxFuture,
};
use tokio::{
    fs,
    sync::RwLock,
};

use crate::{
    Collection,
    CollectionPeek,
    Entry,
    Leaf,
    Result,
};

pub fn fs(path: impl AsRef<Path>) -> Result<impl Leaf + CollectionPeek> {
    let path = path.as_ref();

    let path = path
        .canonicalize()
        .with_context(|| format!("failed to canonicalize path '{path}'", path = path.to_string_lossy()))?;

    Ok(FsEntry {
        location: FsEntryLocation::Root { path: path.clone() },

        content: Arc::new(move |parent| content(parent, path.clone())),
    })
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
    content: Arc<dyn Send + Sync + Fn(Arc<Self>) -> BoxFuture<'static, Result<FsEntryContent>>>,
}

impl fmt::Display for FsEntry {
    fn fmt(&self, writer: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.location {
            FsEntryLocation::Root { path } => write!(writer, "fs:{path}", path = path.to_str().ok_or(fmt::Error)?),
            FsEntryLocation::Child { name, .. } => write!(writer, "{name}"),
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

    async fn as_collection(self: Arc<Self>) -> Option<Arc<dyn Collection>> {
        Some(self)
    }
}

#[async_trait]
impl Leaf for FsEntry {
    async fn read(self: Arc<Self>) -> Result<Bytes> {
        match (self.content)(self.clone()).await? {
            FsEntryContent::Leaf(bytes) => Ok(bytes),

            FsEntryContent::CollectionPeek(_) => {
                bail!(
                    "failed to read '{name}' under '{path}' as it is a directory",
                    name = "foo",
                    path = self.path().to_string_lossy()
                )
            },
        }
    }
}

#[async_trait]
impl Collection for FsEntry {
    async fn entry(self: Arc<Self>, name: &str) -> Result<Option<Arc<dyn Entry>>> {
        for entry in self.list().await?.iter() {
            if entry.name() == Some(name) {
                return Ok(Some(Arc::clone(entry)));
            }
        }

        Ok(None)
    }
}

#[async_trait]
impl CollectionPeek for FsEntry {
    async fn list(self: Arc<Self>) -> Result<Arc<[Arc<dyn Entry>]>> {
        match (self.content)(self.clone()).await? {
            FsEntryContent::CollectionPeek(entries) => Ok(entries),

            FsEntryContent::Leaf(_) => {
                bail!(
                    "failed to read '{name}' under '{path}' as it is a directory",
                    name = "foo",
                    path = self.path().to_string_lossy()
                )
            },
        }
    }
}

fn content(parent: Arc<FsEntry>, path: PathBuf) -> BoxFuture<'static, Result<FsEntryContent>> {
    let content = RwLock::new(None::<Result<FsEntryContent>>);

    async move {
        if let Some(content) = content.read().await.as_ref() {
            return content.clone();
        }

        let result = content_eager(parent.clone(), path).await;

        *content.write().await = Some(result.clone());

        result
    }
    .boxed()
}

async fn content_eager(parent: Arc<FsEntry>, path: PathBuf) -> Result<FsEntryContent> {
    let metadata = fs::metadata(&path)
        .await
        .with_context(|| format!("failed to get metadata of '{path}'", path = path.to_string_lossy()))?;

    if metadata.is_file() || metadata.is_symlink() {
        return content_file_eager(path).await;
    }

    if metadata.is_dir() {
        return content_dir_eager(parent, path).await;
    }

    bail!(
        "unsupported file type of entry at '{path}'",
        path = path.to_string_lossy(),
    );
}

async fn content_file_eager(path: PathBuf) -> Result<FsEntryContent> {
    let bytes = fs::read(&path)
        .await
        .with_context(|| format!("failed to read '{path}'", path = path.to_string_lossy()))?;

    Ok(FsEntryContent::Leaf(Bytes::from(bytes)))
}

async fn content_dir_eager(parent: Arc<FsEntry>, path: PathBuf) -> Result<FsEntryContent> {
    let mut read_dir = fs::read_dir(&path)
        .await
        .with_context(|| format!("failed to list '{path}'", path = path.to_string_lossy()))?;

    let mut entries = Vec::<Arc<dyn Entry>>::new();

    while let Some(entry) = read_dir
        .next_entry()
        .await
        .with_context(|| format!("failed to read entry under '{path}'", path = path.to_string_lossy()))?
    {
        let name = entry.file_name();

        let mut path = path.clone();
        path.push(&name);

        entries.push(Arc::new(FsEntry {
            location: FsEntryLocation::Child {
                parent: parent.clone(),
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

            content: Arc::new(move |parent| content(parent, path.clone())),
        }))
    }

    Ok(FsEntryContent::CollectionPeek(entries.into()))
}
