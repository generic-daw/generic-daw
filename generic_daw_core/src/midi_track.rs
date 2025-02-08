use crate::{Meter, MidiClip, Position};
use audio_graph::{AudioGraphNodeImpl, MixerNode};
use std::sync::Arc;

mod dirty_event;

pub use dirty_event::DirtyEvent;

#[derive(Clone, Debug)]
pub struct MidiTrack {
    /// contains clips of midi patterns
    pub clips: Vec<Arc<MidiClip>>,
    /// information relating to the playback of the arrangement
    pub meter: Arc<Meter>,
    /// volume and pan
    pub node: Arc<MixerNode>,
}

impl AudioGraphNodeImpl for MidiTrack {
    fn fill_buf(&self, _buf_start_sample: usize, _buf: &mut [f32]) {
        unimplemented!()
    }

    fn id(&self) -> audio_graph::NodeId {
        self.node.id()
    }
}

impl MidiTrack {
    #[must_use]
    pub fn new(meter: Arc<Meter>, node: Arc<MixerNode>) -> Self {
        Self {
            clips: Vec::new(),
            meter,
            node,
        }
    }

    #[must_use]
    pub fn len(&self) -> Position {
        self.clips
            .iter()
            .map(|clip| clip.get_global_end())
            .max()
            .unwrap_or_default()
    }
}
