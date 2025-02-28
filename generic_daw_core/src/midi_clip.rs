use crate::{Meter, Position, clip_position::ClipPosition};
use arc_swap::ArcSwap;
use clap_host::{
    NoteBuffers,
    clack_host::{
        events::event_types::{NoteOffEvent, NoteOnEvent},
        prelude::*,
    },
};
use std::sync::{Arc, atomic::Ordering::Acquire};

mod midi_note;

pub use midi_note::{MidiNote, NoteId};

#[derive(Clone, Debug)]
pub struct MidiClip {
    /// the pattern that the clip points to
    ///
    /// swap the internal boxed slice in order to modify the contents
    pub pattern: Arc<ArcSwap<Box<[MidiNote]>>>,
    /// the position of the clip relative to the start of the arrangement
    pub position: ClipPosition,
    /// information relating to the playback of the arrangement
    pub meter: Arc<Meter>,
}

impl MidiClip {
    #[must_use]
    pub fn create(pattern: Arc<ArcSwap<Box<[MidiNote]>>>, meter: Arc<Meter>) -> Arc<Self> {
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

    pub fn gather_events(&self, note_buffers: &mut NoteBuffers, len: usize, steady_time: u32) {
        let global_start = self.position.get_global_start();
        let global_end = self.position.get_global_end();
        let clip_start = self.position.get_clip_start();

        let start_sample = self.meter.sample.load(Acquire);
        let end_sample = start_sample + len;

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
                // TODO: handle notes that we don't see coming

                let start = note
                    .start
                    .in_interleaved_samples(bpm, self.meter.sample_rate);
                if start >= start_sample && start < end_sample {
                    note_buffers.input_events.push(&NoteOnEvent::new(
                        steady_time + (start - start_sample) as u32,
                        Pckn::new(
                            note_buffers.main_input_port,
                            note.channel,
                            note.key,
                            *note.note_id,
                        ),
                        note.velocity,
                    ));
                }

                let end = note.end.in_interleaved_samples(bpm, self.meter.sample_rate);
                if end >= start_sample && end < end_sample {
                    note_buffers.input_events.push(&NoteOffEvent::new(
                        steady_time + (end - start_sample) as u32,
                        Pckn::new(
                            note_buffers.main_input_port,
                            note.channel,
                            note.key,
                            *note.note_id,
                        ),
                        note.velocity,
                    ));
                }
            });
    }
}
