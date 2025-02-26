use crate::{Meter, Position, clip_position::ClipPosition};
use clap_host::{
    NoteBuffers,
    clack_host::{
        events::{
            Match,
            event_types::{NoteOffEvent, NoteOnEvent},
        },
        prelude::*,
    },
};
use std::sync::{Arc, atomic::Ordering::Acquire};

mod midi_note;
mod midi_pattern;

pub use midi_note::MidiNote;
pub use midi_pattern::MidiPattern;

#[derive(Clone, Debug)]
pub struct MidiClip {
    pub pattern: Arc<MidiPattern>,
    /// the position of the clip relative to the start of the arrangement
    pub position: ClipPosition,
    /// information relating to the playback of the arrangement
    pub meter: Arc<Meter>,
}

impl MidiClip {
    #[must_use]
    pub fn create(pattern: Arc<MidiPattern>, meter: Arc<Meter>) -> Arc<Self> {
        let len = pattern.len();

        Arc::new(Self {
            pattern,
            position: ClipPosition::new(Position::ZERO, len, Position::ZERO),
            meter,
        })
    }

    pub fn gather_events(&self, note_buffers: &mut NoteBuffers, len: usize, steady_time: u64) {
        let global_start = self.position.get_global_start();
        let global_end = self.position.get_global_end();
        let clip_start = self.position.get_clip_start();

        let start_sample = self.meter.sample.load(Acquire);
        let end_sample = start_sample + len;

        self.pattern
            .notes()
            .iter()
            .filter_map(|&note| {
                // TODO: handle clips whose patterns start before the arrangement start
                (note + global_start.saturating_sub(clip_start)).clamp(global_start, global_end)
            })
            .for_each(|note| {
                // TODO: handle notes that we don't see coming

                let start = note.start.in_interleaved_samples(&self.meter);
                if start >= start_sample && start < end_sample {
                    note_buffers.input_events.push(&NoteOnEvent::new(
                        (steady_time + (start - start_sample) as u64) as u32,
                        Pckn::new(
                            note_buffers.main_input_port,
                            note.channel,
                            note.note,
                            Match::All,
                        ),
                        note.velocity,
                    ));
                }

                let end = note.end.in_interleaved_samples(&self.meter);
                if end >= start_sample && end < end_sample {
                    note_buffers.input_events.push(&NoteOffEvent::new(
                        (steady_time + (end - start_sample) as u64) as u32,
                        Pckn::new(
                            note_buffers.main_input_port,
                            note.channel,
                            note.note,
                            Match::All,
                        ),
                        note.velocity,
                    ));
                }
            });
    }
}
