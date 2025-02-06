use std::{
    ops::Deref,
    sync::atomic::{AtomicUsize, Ordering::AcqRel},
};

static ID: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct NodeId(usize);

impl NodeId {
    pub fn unique() -> Self {
        Self(ID.fetch_add(1, AcqRel))
    }
}

impl Deref for NodeId {
    type Target = usize;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
