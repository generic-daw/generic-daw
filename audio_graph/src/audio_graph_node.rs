use crate::{AudioGraphNodeImpl, MixerNode};
use std::{
    hash::{Hash, Hasher},
    sync::Arc,
};

#[derive(Clone, Debug)]
pub struct AudioGraphNode(Arc<dyn AudioGraphNodeImpl>);

impl Default for AudioGraphNode {
    fn default() -> Self {
        Self(Arc::new(MixerNode::default()))
    }
}

impl AudioGraphNodeImpl for AudioGraphNode {
    fn fill_buf(&self, buf_start_sample: usize, buf: &mut [f32]) {
        self.0.fill_buf(buf_start_sample, buf);
    }
}

impl PartialEq for AudioGraphNode {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for AudioGraphNode {}

impl Hash for AudioGraphNode {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.0).addr().hash(state);
    }
}

impl<T> From<Arc<T>> for AudioGraphNode
where
    T: AudioGraphNodeImpl + 'static,
{
    fn from(value: Arc<T>) -> Self {
        Self(value as Arc<dyn AudioGraphNodeImpl>)
    }
}
