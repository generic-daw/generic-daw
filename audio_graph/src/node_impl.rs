use crate::NodeId;
use std::{fmt::Debug, ops::Deref};

pub trait NodeImpl<Event>: Debug + Send {
    /// process audio data into `audio` and event data into `events`
    fn process(&self, audio: &mut [f32], events: &mut Vec<Event>);
    /// get the unique `NodeId` of the node
    fn id(&self) -> NodeId;
    /// reset the node to a pre-playback state
    fn reset(&self);
    /// the delay this node introduces
    fn delay(&self) -> usize;
}

impl<T, Event> NodeImpl<Event> for T
where
    T: Debug + Send + Deref<Target: NodeImpl<Event> + Sized>,
{
    fn process(&self, audio: &mut [f32], events: &mut Vec<Event>) {
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
