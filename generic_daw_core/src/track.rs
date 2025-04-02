use crate::{Meter, MixerNode, Position, clip::Clip, event::Event};
use audio_graph::{NodeId, NodeImpl};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct Track {
    pub clips: Vec<Clip>,
    /// information relating to the playback of the arrangement
    pub meter: Arc<Meter>,
    /// volume, pan and plugins
    pub node: Arc<MixerNode>,
}

impl NodeImpl<Event> for Track {
    fn process(&self, audio: &mut [f32], events: &mut Vec<Event>) {
        for clip in &self.clips {
            clip.process(audio, events);
        }

        self.node.process(audio, events);
    }

    fn id(&self) -> NodeId {
        self.node.id()
    }

    fn reset(&self) {
        self.node.reset();
    }

    fn delay(&self) -> usize {
        self.node.delay()
    }
}

impl Track {
    #[must_use]
    pub fn new(meter: Arc<Meter>) -> Self {
        Self {
            clips: Vec::new(),
            meter,
            node: Arc::default(),
        }
    }

    #[must_use]
    pub fn len(&self) -> Position {
        self.clips
            .iter()
            .map(|clip| clip.position().get_global_end())
            .max()
            .unwrap_or_default()
    }
}
