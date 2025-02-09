use crate::NodeId;
use std::{fmt::Debug, ops::Deref};

pub trait AudioGraphNodeImpl: Debug + Send {
    /// `buf` contains the summed data from all dependencies in the graph
    fn fill_buf(&self, buf_start_sample: usize, buf: &mut [f32]);
    /// get the `NodeId` of the node
    fn id(&self) -> NodeId;
}

impl<C, T> AudioGraphNodeImpl for C
where
    C: Debug + Send + Deref<Target = T>,
    T: AudioGraphNodeImpl,
{
    fn fill_buf(&self, buf_start_sample: usize, buf: &mut [f32]) {
        (**self).fill_buf(buf_start_sample, buf);
    }

    fn id(&self) -> NodeId {
        (**self).id()
    }
}
