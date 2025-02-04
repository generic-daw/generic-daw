use std::sync::atomic::{AtomicUsize, Ordering::AcqRel};

static ID: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct NodeId(usize);

impl NodeId {
    pub fn unique() -> Self {
        Self(ID.fetch_add(1, AcqRel))
    }
}
