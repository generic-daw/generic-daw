use crate::{Meter, MidiClip, MixerNode, Position};
use audio_graph::{AudioGraphNodeImpl, NodeId};
use clap_host::{
    AudioProcessor,
    clack_host::{
        events::event_types::{NoteChokeEvent, NoteOffEvent},
        prelude::*,
    },
};
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

        let steady_time = lock.steady_time() as u32;
        if self.meter.playing.load(Acquire) {
            for clip in &self.clips {
                clip.gather_events(&mut lock.note_buffers, buf.len(), steady_time);
            }
        } else {
            lock.note_buffers
                .input_events
                .push(NoteOffEvent::new(steady_time, Pckn::match_all(), 1.0).as_unknown());
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

        let steady_time = lock.steady_time() as u32;
        lock.note_buffers
            .input_events
            .push(&NoteChokeEvent::new(steady_time, Pckn::match_all()));
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
