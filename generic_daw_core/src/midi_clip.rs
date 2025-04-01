use crate::{Meter, Position, clip_position::ClipPosition};
use arc_swap::ArcSwap;
use clap_host::Event;
use std::sync::{Arc, atomic::Ordering::Acquire};

mod midi_key;
mod midi_note;

pub use midi_key::{Key, MidiKey};
pub use midi_note::MidiNote;

#[derive(Clone, Debug)]
pub struct MidiClip {
    /// the pattern that the clip points to
    ///
    /// swap the internal boxed slice in order to modify the contents
    pub pattern: Arc<ArcSwap<Vec<MidiNote>>>,
    /// the position of the clip relative to the start of the arrangement
    pub position: ClipPosition,
    /// information relating to the playback of the arrangement
    pub meter: Arc<Meter>,
}

impl MidiClip {
    #[must_use]
    pub fn create(pattern: Arc<ArcSwap<Vec<MidiNote>>>, meter: Arc<Meter>) -> Arc<Self> {
        let len = pattern
            .load()
            .iter()
            .map(|note| note.end)
            .max()
            .unwrap_or_default();

        Arc::new(Self {
            pattern,
            position: ClipPosition::new(Position::ZERO, len, Position::ZERO),
            meter,
        })
    }

    pub fn process(&self, audio: &[f32], events: &mut Vec<Event>) {
        let global_start = self.position.get_global_start();
        let global_end = self.position.get_global_end();
        let clip_start = self.position.get_clip_start();

        let start_sample = self.meter.sample.load(Acquire);
        let end_sample = start_sample + audio.len();

        let bpm = self.meter.bpm.load(Acquire);

        self.pattern
            .load()
            .iter()
            .filter_map(|&note| {
                (note + global_start)
                    .saturating_sub(clip_start)
                    .and_then(|note| note.clamp(global_start, global_end))
            })
            .for_each(|note| {
                // TODO: handle starting and stopping notes in the middle of their duration

                let start = note.start.in_samples(bpm, self.meter.sample_rate);
                if start >= start_sample && start < end_sample {
                    events.push(Event::On {
                        time: (start - start_sample) as u32 / 2,
                        channel: note.channel,
                        key: note.key.0,
                        velocity: note.velocity,
                    });
                }

                let end = note.end.in_samples(bpm, self.meter.sample_rate);
                if end >= start_sample && end < end_sample {
                    events.push(Event::Off {
                        time: (end - start_sample) as u32 / 2,
                        channel: note.channel,
                        key: note.key.0,
                        velocity: note.velocity,
                    });
                }
            });
    }
}
