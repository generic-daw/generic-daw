use crate::{Master, MixerNode, Track};
use audio_graph::AudioGraphNodeImpl;
use clap_host::Event;
use std::sync::Arc;

#[derive(Debug)]
pub enum AudioGraphNode {
    Master(Master),
    MixerNode(Arc<MixerNode>),
    Track(Track),
}

impl AudioGraphNodeImpl<f32, Event> for AudioGraphNode {
    fn process(&self, audio: &mut [f32], events: &mut Vec<Event>) {
        match self {
            Self::Master(node) => node.process(audio, events),
            Self::MixerNode(node) => node.process(audio, events),
            Self::Track(node) => node.process(audio, events),
        }
    }

    fn id(&self) -> audio_graph::NodeId {
        match self {
            Self::Master(node) => node.id(),
            Self::MixerNode(node) => node.id(),
            Self::Track(node) => node.id(),
        }
    }

    fn reset(&self) {
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

impl From<Arc<MixerNode>> for AudioGraphNode {
    fn from(value: Arc<MixerNode>) -> Self {
        Self::MixerNode(value)
    }
}

impl From<Track> for AudioGraphNode {
    fn from(value: Track) -> Self {
        Self::Track(value)
    }
}
