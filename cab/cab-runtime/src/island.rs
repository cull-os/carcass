use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use cab_why::Result;

#[async_trait]
pub trait Entry: Send + Sync + 'static {
    // /// Returns the name of this entry. This should return the key used in
    // /// creating this entry, if it was created using [`Entry::get`].
    // fn name(&self) -> Option<&str> {
    //     None
    // }

    /// Returns the parent of this entry. If it returns [`None`], it is an
    /// island, also known as a virtual root.
    fn parent(&self) -> Option<Arc<dyn Entry>> {
        None
    }

    /// Reads the contents of this entry.
    async fn read(self: Arc<Self>) -> Result<Bytes> {
        // bail!("{this} is not a leaf", this = &*self)
        todo!()
    }

    /// Fetches a child entry by name.
    async fn get(self: Arc<Self>, _name: &str) -> Result<Option<Arc<dyn Entry>>> {
        // bail!("{this} is not a collection", this = &*self)
        todo!()
    }

    /// Lists the children of this entry.
    async fn list(self: Arc<Self>) -> Result<Arc<[Arc<dyn Entry>]>> {
        // bail!("{this} is not a listable collection", this = &*self)
        todo!()
    }
}
