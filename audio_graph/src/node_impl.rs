use crate::{EventImpl, NodeId};
use std::fmt::Debug;

pub trait NodeImpl: Debug + Send {
    type Event: EventImpl;
    type State;
    /// process audio data into `audio` and event data into `events`
    fn process(&mut self, state: &Self::State, audio: &mut [f32], events: &mut Vec<Self::Event>);
    /// get the unique `NodeId` of the node
    fn id(&self) -> NodeId;
    /// reset the node to a pre-playback state
    fn reset(&mut self);
    /// the delay this node introduces
    fn delay(&self) -> usize;
}
