use crate::{Meter, MidiClip, Position};
use audio_graph::{AudioGraphNodeImpl, MixerNode};
use clap_host::{
    AudioProcessor,
    clack_host::prelude::{InputEvents, OutputEvents},
};
use std::sync::{Arc, Mutex};

mod dirty_event;

pub use dirty_event::DirtyEvent;

#[derive(Clone, Debug)]
pub struct MidiTrack {
    host_audio_processor: Arc<Mutex<AudioProcessor>>,
    /// contains clips of midi patterns
    pub clips: Vec<Arc<MidiClip>>,
    /// information relating to the playback of the arrangement
    pub meter: Arc<Meter>,
    /// volume and pan
    pub node: Arc<MixerNode>,
}

impl AudioGraphNodeImpl for MidiTrack {
    fn fill_buf(&self, buf_start_sample: usize, buf: &mut [f32]) {
        self.host_audio_processor
            .try_lock()
            .expect("this is only locked from the audio thread")
            .process(buf, &InputEvents::empty(), &mut OutputEvents::void());

        self.node.fill_buf(buf_start_sample, buf);
    }

    fn id(&self) -> audio_graph::NodeId {
        self.node.id()
    }
}

impl MidiTrack {
    #[must_use]
    pub fn new(meter: Arc<Meter>, audio_processor: AudioProcessor) -> Self {
        Self {
            host_audio_processor: Arc::new(Mutex::new(audio_processor)),
            clips: Vec::new(),
            meter,
            node: Arc::default(),
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
