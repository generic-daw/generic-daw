use crate::{Action, Master, Mixer, Track, daw_ctx::State, event::Event};
use audio_graph::NodeImpl;

#[derive(Debug)]
pub enum AudioGraphNode {
    Master(Master),
    Mixer(Mixer),
    Track(Track),
}

impl NodeImpl for AudioGraphNode {
    type Event = Event;
    type State = State;

    fn process(&mut self, state: &Self::State, audio: &mut [f32], events: &mut Vec<Self::Event>) {
        match self {
            Self::Master(node) => node.process(state, audio, events),
            Self::Mixer(node) => node.process(state, audio, events),
            Self::Track(node) => node.process(state, audio, events),
        }
    }

    fn id(&self) -> audio_graph::NodeId {
        match self {
            Self::Master(node) => node.id(),
            Self::Mixer(node) => node.id(),
            Self::Track(node) => node.id(),
        }
    }

    fn reset(&mut self) {
        match self {
            Self::Master(node) => node.reset(),
            Self::Mixer(node) => node.reset(),
            Self::Track(node) => node.reset(),
        }
    }

    fn delay(&self) -> usize {
        match self {
            Self::Master(node) => node.delay(),
            Self::Mixer(node) => node.delay(),
            Self::Track(node) => node.delay(),
        }
    }
}

impl AudioGraphNode {
    pub fn apply(&mut self, action: Action) {
        match self {
            Self::Master(node) => node.apply(action),
            Self::Mixer(node) => node.apply(action),
            Self::Track(node) => node.apply(action),
        }
    }
}

impl From<Master> for AudioGraphNode {
    fn from(value: Master) -> Self {
        Self::Master(value)
    }
}

impl From<Mixer> for AudioGraphNode {
    fn from(value: Mixer) -> Self {
        Self::Mixer(value)
    }
}

impl From<Track> for AudioGraphNode {
    fn from(value: Track) -> Self {
        Self::Track(value)
    }
}
