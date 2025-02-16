use std::{
    fmt,
    sync::Arc,
};

use async_once_cell::OnceCell;
use async_trait::async_trait;
use bytes::Bytes;
use cab_error::Contextful as _;
use tokio::io::{
    self,
    AsyncReadExt as _,
};

use crate::{
    Entry,
    Leaf,
    Result,
};

pub fn stdin() -> impl Leaf {
    Stdin {
        content: OnceCell::new(),
    }
}

struct Stdin {
    content: OnceCell<Result<Bytes>>,
}

impl fmt::Display for Stdin {
    fn fmt(&self, writer: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(writer, "stdin")
    }
}

#[async_trait]
impl Entry for Stdin {
    async fn as_leaf(self: Arc<Self>) -> Option<Arc<dyn Leaf>> {
        Some(self)
    }
}

#[async_trait]
impl Leaf for Stdin {
    async fn read(self: Arc<Self>) -> Result<Bytes> {
        self.content().await
    }
}

impl Stdin {
    async fn content(&self) -> Result<Bytes> {
        self.content
            .get_or_init(async {
                let mut buffer = Vec::new();

                io::stdin()
                    .read_to_end(&mut buffer)
                    .await
                    .context("failed to read from stdin")?;

                Ok(Bytes::from(buffer))
            })
            .await
            .clone()
    }
}
