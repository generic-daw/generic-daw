use crate::{Meter, MidiClip, Position};
use audio_graph::{AudioGraphNodeImpl, MixerNode, NodeId};
use clap_host::AudioProcessor;
use std::sync::{Arc, Mutex};

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
            .process(buf);

        self.node.fill_buf(buf_start_sample, buf);
    }

    fn id(&self) -> NodeId {
        self.node.id()
    }

    fn reset(&self) {
        self.host_audio_processor
            .try_lock()
            .expect("this is only locked from the audio thread")
            .reset();
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
            .map(|clip| clip.position.get_global_end())
            .max()
            .unwrap_or_default()
    }
}
