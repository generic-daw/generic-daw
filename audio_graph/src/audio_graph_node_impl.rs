use crate::NodeId;
use std::{fmt::Debug, ops::Deref};

pub trait AudioGraphNodeImpl: Debug + Send {
    /// process audio data into `buf`
    ///
    /// `buf` contains the summed data from all dependencies in the graph.
    fn fill_buf(&self, buf: &mut [f32]);
    /// get the unique `NodeId` of the node
    fn id(&self) -> NodeId;
    /// reset the node to a pre-playback state
    fn reset(&self) {}
}

impl<T> AudioGraphNodeImpl for T
where
    T: Debug + Send + Deref<Target: AudioGraphNodeImpl + Sized>,
{
    fn fill_buf(&self, buf: &mut [f32]) {
        (**self).fill_buf(buf);
    }

    fn id(&self) -> NodeId {
        (**self).id()
    }

    fn reset(&self) {
        (**self).reset();
    }
}
