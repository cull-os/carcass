#![feature(trait_alias, let_chains)]

use std::{
    fmt,
    sync::Arc,
};

use async_trait::async_trait;
use bytes::Bytes;
use cab_error::Result;

mod fs;
pub use fs::*;

mod stdin;
use futures::future;
pub use stdin::*;

type Shared<T> = future::Shared<future::BoxFuture<'static, T>>;

#[async_trait]
pub trait Entry: fmt::Display + Send + Sync + 'static {
    fn name(&self) -> Option<&str> {
        None
    }

    fn parent(&self) -> Option<Arc<dyn Collection>> {
        None
    }

    async fn as_leaf(self: Arc<Self>) -> Option<Arc<dyn Leaf>> {
        None
    }

    async fn as_collection(self: Arc<Self>) -> Option<Arc<dyn Collection>> {
        None
    }
}

impl dyn Entry {
    pub fn display(self: Arc<Self>, writer: &mut dyn fmt::Write) -> fmt::Result {
        let mut entries = vec![self];

        while let Some(parent) = entries.last().unwrap().parent() {
            entries.push(parent);
        }

        for (index, entry) in entries.iter().rev().enumerate() {
            if index == 0 {
                write!(writer, "<{entry}>")?;
            } else {
                write!(writer, "/{entry}")?;
            }
        }

        Ok(())
    }
}

#[async_trait]
pub trait Leaf: Entry {
    async fn read(self: Arc<Self>) -> Result<Bytes>;
}

#[async_trait]
pub trait Collection: Entry {
    async fn entry(self: Arc<Self>, name: &str) -> Result<Option<Arc<dyn Entry>>>;
}

#[async_trait]
pub trait CollectionPeek: Collection {
    async fn list(self: Arc<Self>) -> Result<Arc<[Arc<dyn Entry>]>>;
}
