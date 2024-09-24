mod dirty_event;
pub use dirty_event::{AtomicDirtyEvent, DirtyEvent};

mod midi_note;
pub use midi_note::MidiNote;

mod midi_pattern;
pub use midi_pattern::MidiPattern;

use crate::generic_back::{Arrangement, Position};
use std::{
    cmp::Ordering,
    sync::{atomic::Ordering::SeqCst, Arc},
};

#[derive(Debug)]
pub struct MidiClip {
    pub pattern: MidiPattern,
    /// the start of the clip relative to the start of the arrangement
    global_start: Position,
    /// the end of the clip relative to the start of the arrangement
    global_end: Position,
    /// the start of the clip relative to the start of the pattern
    pattern_start: Position,
    pub arrangement: Arc<Arrangement>,
}

impl MidiClip {
    pub fn new(pattern: MidiPattern, arrangement: Arc<Arrangement>) -> Self {
        let len = pattern.len();
        Self {
            pattern,
            global_start: Position::default(),
            global_end: Position::from_interleaved_samples(len, &arrangement.meter),
            pattern_start: Position::default(),
            arrangement,
        }
    }

    pub const fn get_global_start(&self) -> Position {
        self.global_start
    }

    pub const fn get_global_end(&self) -> Position {
        self.global_end
    }

    pub fn trim_start_to(&mut self, pattern_start: Position) {
        match self.pattern_start.cmp(&pattern_start) {
            Ordering::Less => {
                self.global_start += pattern_start - self.pattern_start;
            }
            Ordering::Equal => {}
            Ordering::Greater => {
                self.global_start -= self.pattern_start - pattern_start;
            }
        }
        self.pattern_start = pattern_start;
        self.pattern.dirty.store(DirtyEvent::NoteReplaced, SeqCst);
        assert!(self.global_start <= self.global_end);
    }

    pub fn trim_end_to(&mut self, global_end: Position) {
        self.global_end = global_end;
        self.pattern.dirty.store(DirtyEvent::NoteReplaced, SeqCst);
        assert!(self.global_start <= self.global_end);
    }

    pub fn move_to(&mut self, global_start: Position) {
        match self.global_start.cmp(&global_start) {
            Ordering::Less => {
                self.global_end += global_start - self.global_start;
            }
            Ordering::Equal => {}
            Ordering::Greater => {
                self.global_end -= self.global_start - global_start;
            }
        }
        self.global_start = global_start;
        self.pattern.dirty.store(DirtyEvent::NoteReplaced, SeqCst);
    }

    pub(in crate::generic_back) fn get_global_midi(&self) -> Vec<Arc<MidiNote>> {
        let global_start = self
            .global_start
            .in_interleaved_samples(&self.arrangement.meter);
        let global_end = self
            .global_end
            .in_interleaved_samples(&self.arrangement.meter);
        self.pattern
            .notes
            .iter()
            .map(|note| {
                Arc::new(MidiNote {
                    channel: note.channel,
                    note: note.note,
                    velocity: note.velocity,
                    local_start: (note.local_start + global_start - global_end)
                        .clamp(global_start, global_end),
                    local_end: (note.local_end + global_start - global_end)
                        .clamp(global_start, global_end),
                })
            })
            .filter(|note| note.local_start != note.local_end)
            .collect()
    }
}
