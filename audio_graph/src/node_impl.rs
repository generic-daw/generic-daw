use crate::{EventImpl, NodeId};
use std::fmt::Debug;

pub trait NodeImpl: Debug + Send {
    type Action;
    type Event: EventImpl;
    type State;
    /// apply an action to this node
    fn apply(&mut self, action: Self::Action);
    /// process audio data into `audio` and event data into `events`
    fn process(&mut self, state: &Self::State, audio: &mut [f32], events: &mut Vec<Self::Event>);
    /// reset the node to a pre-playback state
    fn reset(&mut self);
    /// get the unique `NodeId` of the node
    fn id(&self) -> NodeId;
    /// the delay this node introduces
    fn delay(&self) -> usize;
}
