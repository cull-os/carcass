//! Island implementations and dyn-compatible traits.
//!
//! An island is a virtual filesystem root or leaf, also known as an [`Entry`].
//!
//! An entry may be a [`Leaf`] or a [`Collection`] and its derivatives, or both.
use std::{
    fmt,
    sync::Arc,
};

use async_trait::async_trait;
use bytes::Bytes;
use cab_why::Result;

mod blob;
pub use blob::*;

mod fs;
pub use fs::*;

mod stdin;
pub use stdin::*;

/// An island entry. Entries which don't have parents are roots. Roots are not
/// guaranteed to be collections, as things like `<fs::/etc/resolv.conf>` are
/// leafs without parents.
#[async_trait]
pub trait Entry: fmt::Display + Send + Sync + 'static {
    /// Returns the name of this entry. This should return the key used in
    /// creating this node, if it was created using [`Collection::entry`].
    fn name(&self) -> Option<&str> {
        None
    }

    /// Returns the parent of this entry. If it returns [`None`], it is a root.
    fn parent(&self) -> Option<Arc<dyn Collection>> {
        None
    }

    /// Tries to use this entry as a [`Leaf`].
    async fn as_leaf(self: Arc<Self>) -> Option<Arc<dyn Leaf>> {
        None
    }

    /// Tries to use this entry as a [`Collection`].
    async fn as_collection(self: Arc<Self>) -> Option<Arc<dyn Collection>> {
        None
    }

    /// Tries to use this entry as a [`CollectionList`].
    async fn as_collection_list(self: Arc<Self>) -> Option<Arc<dyn CollectionList>> {
        None
    }
}

/// Converts the given object wrapped in an [`Arc`] that is castable to an
/// [`Entry`] to a [`fmt::Display`].
#[macro_export]
macro_rules! display {
    ($entry:expr) => {{
        let entry: ::std::sync::Arc<dyn $crate::Entry> = $entry.clone();
        entry.display()
    }};
}

impl dyn Entry {
    /// Converts this entry to a displayable type.
    pub fn display(self: Arc<Self>) -> impl fmt::Display {
        struct EntryDisplay(Arc<dyn Entry>);

        impl fmt::Display for EntryDisplay {
            fn fmt(&self, writer: &mut fmt::Formatter<'_>) -> fmt::Result {
                let mut entries = vec![self.0.clone()];

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

        EntryDisplay(self)
    }
}

/// A leaf. Leaves have contents that can be read. An entry being a leaf doesn't
/// disqualify it from being a collection.
#[async_trait]
pub trait Leaf: Entry {
    /// Reads the contents of this leaf.
    async fn read(self: Arc<Self>) -> Result<Bytes>;
}

/// A collection. Collections have children entries that can be accessed with a
/// name key.
#[async_trait]
pub trait Collection: Entry {
    /// Fetches a child entry of this collection by name.
    async fn entry(self: Arc<Self>, name: &str) -> Result<Option<Arc<dyn Entry>>>;
}

/// A listable collection. Basically a collection but with the ability to list
/// its children.
#[async_trait]
pub trait CollectionList: Collection {
    /// Lists the children of this collection.
    async fn list(self: Arc<Self>) -> Result<Arc<[Arc<dyn Entry>]>>;
}
