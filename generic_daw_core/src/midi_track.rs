use crate::{Meter, MidiClip, MixerNode, Position};
use audio_graph::{AudioGraphNodeImpl, NodeId};
use clap_host::AudioProcessor;
use std::sync::{Arc, Mutex, atomic::Ordering::Acquire};

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
    fn fill_buf(&self, buf: &mut [f32]) {
        let mut lock = self
            .host_audio_processor
            .try_lock()
            .expect("this is only locked from the audio thread");

        if self.meter.playing.load(Acquire) {
            for clip in &self.clips {
                clip.gather_events(&mut lock.note_buffers, buf.len());
            }
        }

        lock.process(buf, 1.0);

        drop(lock);

        self.node.fill_buf(buf);
    }

    fn id(&self) -> NodeId {
        self.node.id()
    }

    fn reset(&self) {
        let mut lock = self
            .host_audio_processor
            .try_lock()
            .expect("this is only locked from the audio thread");

        lock.reset();

        lock.note_buffers.all_notes_off(0, 0);
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
