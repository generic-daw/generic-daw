use crate::{AudioClip, Meter, MixerNode, Position};
use audio_graph::{AudioGraphNodeImpl, NodeId};
use std::sync::{Arc, atomic::Ordering::Acquire};

#[derive(Clone, Debug)]
pub struct AudioTrack {
    /// contains clips of audio samples
    pub clips: Vec<Arc<AudioClip>>,
    /// information relating to the playback of the arrangement
    pub meter: Arc<Meter>,
    /// volume and pan
    pub node: Arc<MixerNode>,
}

impl AudioGraphNodeImpl for AudioTrack {
    fn fill_buf(&self, buf: &mut [f32]) {
        if self.meter.playing.load(Acquire) {
            self.clips.iter().for_each(|clip| clip.fill_buf(buf));
        }

        self.node.fill_buf(buf);
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

impl AudioTrack {
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
            .map(|clip| clip.position.get_global_end())
            .max()
            .unwrap_or_default()
    }
}
