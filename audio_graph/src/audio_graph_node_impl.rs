use crate::NodeId;
use std::{fmt::Debug, ops::Deref};

pub trait AudioGraphNodeImpl<S, E>: Debug + Send {
    /// process audio data into `audio` and event data into `events`
    fn process(&self, audio: &mut [S], events: &mut Vec<E>);
    /// get the unique `NodeId` of the node
    fn id(&self) -> NodeId;
    /// reset the node to a pre-playback state
    fn reset(&self);
    /// the delay this node introduces
    fn delay(&self) -> usize;
}

impl<T, S, E> AudioGraphNodeImpl<S, E> for T
where
    T: Debug + Send + Deref<Target: AudioGraphNodeImpl<S, E> + Sized>,
{
    fn process(&self, audio: &mut [S], events: &mut Vec<E>) {
        (**self).process(audio, events);
    }

    fn id(&self) -> NodeId {
        (**self).id()
    }

    fn reset(&self) {
        (**self).reset();
    }

    fn delay(&self) -> usize {
        (**self).delay()
    }
}
