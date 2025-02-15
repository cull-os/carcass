use std::{
    fmt,
    sync::Arc,
};

use async_trait::async_trait;
use bytes::Bytes;
use cab_error::Contextful as _;
use futures::FutureExt as _;
use tokio::io::{
    self,
    AsyncReadExt as _,
};

use crate::{
    Entry,
    Leaf,
    Result,
    Shared,
};

pub fn stdin() -> impl Leaf {
    Stdin(
        async {
            let mut buffer = Vec::new();

            io::stdin()
                .read_to_end(&mut buffer)
                .await
                .context("failed to read from stdin")?;

            Ok(Bytes::from(buffer))
        }
        .boxed()
        .shared(),
    )
}

struct Stdin(Shared<Result<Bytes>>);

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
        self.0.clone().await
    }
}
