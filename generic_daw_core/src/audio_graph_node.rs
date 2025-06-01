use crate::{Action, Master, MixerNode, Track, daw_ctx::State, event::Event};
use audio_graph::NodeImpl;

#[derive(Debug)]
pub enum AudioGraphNode {
    Master(Master),
    MixerNode(MixerNode),
    Track(Track),
}

impl NodeImpl for AudioGraphNode {
    type Action = Action;
    type Event = Event;
    type State = State;

    fn apply(&mut self, action: Self::Action) {
        match self {
            Self::Master(node) => node.apply(action),
            Self::MixerNode(node) => node.apply(action),
            Self::Track(node) => node.apply(action),
        }
    }

    fn process(&mut self, state: &Self::State, audio: &mut [f32], events: &mut Vec<Self::Event>) {
        match self {
            Self::Master(node) => node.process(state, audio, events),
            Self::MixerNode(node) => node.process(state, audio, events),
            Self::Track(node) => node.process(state, audio, events),
        }
    }

    fn id(&self) -> audio_graph::NodeId {
        match self {
            Self::Master(node) => node.id(),
            Self::MixerNode(node) => node.id(),
            Self::Track(node) => node.id(),
        }
    }

    fn reset(&mut self) {
        match self {
            Self::Master(node) => node.reset(),
            Self::MixerNode(node) => node.reset(),
            Self::Track(node) => node.reset(),
        }
    }

    fn delay(&self) -> usize {
        match self {
            Self::Master(node) => node.delay(),
            Self::MixerNode(node) => node.delay(),
            Self::Track(node) => node.delay(),
        }
    }
}

impl From<Master> for AudioGraphNode {
    fn from(value: Master) -> Self {
        Self::Master(value)
    }
}

impl From<MixerNode> for AudioGraphNode {
    fn from(value: MixerNode) -> Self {
        Self::MixerNode(value)
    }
}

impl From<Track> for AudioGraphNode {
    fn from(value: Track) -> Self {
        Self::Track(value)
    }
}
