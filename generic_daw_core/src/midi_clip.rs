use crate::{Meter, Position, clip_position::ClipPosition, event::Event};
use arc_swap::ArcSwap;
use generic_daw_utils::NoDebug;
use std::{
    cmp::Ordering,
    iter::repeat_n,
    sync::{Arc, Mutex, atomic::Ordering::Acquire},
};

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
    notes: NoDebug<Arc<Mutex<[[u8; 16]; 128]>>>,
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
            notes: Arc::new(Mutex::new([[0; 16]; 128])).into(),
        })
    }

    pub fn process(&self, audio: &[f32], events: &mut Vec<Event>) {
        let global_start = self.position.get_global_start();
        let global_end = self.position.get_global_end();
        let clip_start = self.position.get_clip_start();

        let playing = self.meter.playing.load(Acquire);
        let bpm = self.meter.bpm.load(Acquire);

        let start_sample = self.meter.sample.load(Acquire);
        let end_sample = start_sample + audio.len();

        // how many notes should currently be playing
        let mut notes = [[0u8; 16]; 128];

        if playing {
            self.pattern
                .load()
                .iter()
                .filter_map(|&note| {
                    (note + global_start)
                        .saturating_sub(clip_start)
                        .and_then(|note| note.clamp(global_start, global_end))
                })
                .for_each(|note| {
                    let start = note.start.in_samples(bpm, self.meter.sample_rate);
                    let end = note.end.in_samples(bpm, self.meter.sample_rate);

                    if start < start_sample && end >= start_sample {
                        notes[note.key.0 as usize][note.channel as usize] += 1;
                    }
                });
        }

        // how many notes are currently playing
        let mut lock = self
            .notes
            .try_lock()
            .expect("this is only locked from the audio thread");

        lock.iter()
            .copied()
            .zip(notes)
            .enumerate()
            .flat_map(|(a, (b, c))| b.into_iter().zip(c).enumerate().map(move |b| (a, b)))
            .for_each(|(key, (channel, (before, after)))| {
                // start or stop any difference in the number of playing notes
                //
                // this happens when toggling playback in the middle of a note,
                // or when adding a note that stretches over the playhead

                let event = match before.cmp(&after) {
                    Ordering::Equal => return,
                    Ordering::Less => Event::On {
                        time: 0,
                        channel: channel as u8,
                        key: key as u8,
                        velocity: 1.0,
                    },
                    Ordering::Greater => Event::Off {
                        time: 0,
                        channel: channel as u8,
                        key: key as u8,
                        velocity: 1.0,
                    },
                };

                events.extend(repeat_n(event, before.abs_diff(after) as usize));
            });

        if playing {
            self.pattern
                .load()
                .iter()
                .filter_map(|&note| {
                    (note + global_start)
                        .saturating_sub(clip_start)
                        .and_then(|note| note.clamp(global_start, global_end))
                })
                .for_each(|note| {
                    let start = note.start.in_samples(bpm, self.meter.sample_rate);
                    let end = note.end.in_samples(bpm, self.meter.sample_rate);

                    if start >= start_sample && start < end_sample {
                        events.push(Event::On {
                            time: (start - start_sample) as u32 / 2,
                            channel: note.channel,
                            key: note.key.0,
                            velocity: note.velocity,
                        });

                        // this note will be playing in the next callback
                        notes[note.key.0 as usize][note.channel as usize] += 1;
                    }

                    if end >= start_sample && end < end_sample {
                        events.push(Event::Off {
                            time: (end - start_sample) as u32 / 2,
                            channel: note.channel,
                            key: note.key.0,
                            velocity: note.velocity,
                        });

                        // this note won't be playing in the next callback
                        notes[note.key.0 as usize][note.channel as usize] -= 1;
                    }
                });
        }

        *lock = notes;
    }
}
