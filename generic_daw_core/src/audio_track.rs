use crate::{AudioClip, Meter, Position};
use audio_graph::{AudioGraphNodeImpl, MixerNode};
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
    fn fill_buf(&self, buf_start_sample: usize, buf: &mut [f32]) {
        if !self.meter.playing.load(Acquire) {
            return;
        }

        self.clips
            .iter()
            .for_each(|clip| clip.fill_buf(buf_start_sample, buf));

        self.node.fill_buf(buf_start_sample, buf);
    }

    fn id(&self) -> audio_graph::NodeId {
        self.node.id()
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
