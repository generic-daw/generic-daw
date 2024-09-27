mod dirty_event;
pub use dirty_event::{AtomicDirtyEvent, DirtyEvent};

mod midi_note;
pub use midi_note::MidiNote;

mod midi_pattern;
pub use midi_pattern::MidiPattern;

use crate::generic_back::{Arrangement, Position, TrackClip};
use std::{
    cmp::Ordering,
    sync::{atomic::Ordering::SeqCst, Arc, RwLock},
};

#[derive(Debug)]
pub struct MidiClip {
    pub pattern: MidiPattern,
    /// the start of the clip relative to the start of the arrangement
    global_start: RwLock<Position>,
    /// the end of the clip relative to the start of the arrangement
    global_end: RwLock<Position>,
    /// the start of the clip relative to the start of the pattern
    pattern_start: RwLock<Position>,
    pub arrangement: Arc<Arrangement>,
}

impl MidiClip {
    pub fn create(pattern: MidiPattern, arrangement: Arc<Arrangement>) -> Arc<TrackClip> {
        let len = pattern.len();
        Arc::new(TrackClip::Midi(Self {
            pattern,
            global_start: RwLock::default(),
            global_end: RwLock::new(Position::from_interleaved_samples(len, &arrangement.meter)),
            pattern_start: RwLock::default(),
            arrangement,
        }))
    }

    pub fn get_global_start(&self) -> Position {
        *self.global_start.read().unwrap()
    }

    pub fn get_global_end(&self) -> Position {
        *self.global_end.read().unwrap()
    }

    pub fn trim_start_to(&self, pattern_start: Position) {
        let cmp = self.pattern_start.read().unwrap().cmp(&pattern_start);
        match cmp {
            Ordering::Less => {
                *self.global_start.write().unwrap() +=
                    pattern_start - *self.pattern_start.read().unwrap();
            }
            Ordering::Equal => {}
            Ordering::Greater => {
                *self.global_start.write().unwrap() -=
                    *self.pattern_start.read().unwrap() - pattern_start;
            }
        }
        *self.pattern_start.write().unwrap() = pattern_start;
        self.pattern.dirty.store(DirtyEvent::NoteReplaced, SeqCst);
        assert!(*self.global_start.read().unwrap() <= *self.global_end.read().unwrap());
    }

    pub fn trim_end_to(&self, global_end: Position) {
        *self.global_end.write().unwrap() = global_end;
        self.pattern.dirty.store(DirtyEvent::NoteReplaced, SeqCst);
        assert!(*self.global_start.read().unwrap() <= *self.global_end.read().unwrap());
    }

    pub fn move_to(&self, global_start: Position) {
        let cmp = self.global_start.read().unwrap().cmp(&global_start);
        match cmp {
            Ordering::Less => {
                *self.global_end.write().unwrap() +=
                    global_start - *self.global_start.read().unwrap();
            }
            Ordering::Equal => {}
            Ordering::Greater => {
                *self.global_end.write().unwrap() -=
                    *self.global_start.read().unwrap() - global_start;
            }
        }
        *self.global_start.write().unwrap() = global_start;
        self.pattern.dirty.store(DirtyEvent::NoteReplaced, SeqCst);
    }

    pub(in crate::generic_back) fn get_global_midi(&self) -> Vec<Arc<MidiNote>> {
        let global_start = self
            .global_start
            .read()
            .unwrap()
            .in_interleaved_samples(&self.arrangement.meter);
        let global_end = self
            .global_end
            .read()
            .unwrap()
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
