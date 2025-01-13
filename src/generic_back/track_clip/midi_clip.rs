use crate::generic_back::{DirtyEvent, Meter, Position, TrackClip};
use atomig::Atomic;
use std::sync::{atomic::Ordering::SeqCst, Arc};

pub use midi_note::MidiNote;
pub use midi_pattern::MidiPattern;

mod midi_note;
mod midi_pattern;

#[derive(Debug)]
pub struct MidiClip {
    pub pattern: Arc<MidiPattern>,
    /// the start of the clip relative to the start of the arrangement
    global_start: Atomic<Position>,
    /// the end of the clip relative to the start of the arrangement
    global_end: Atomic<Position>,
    /// the start of the clip relative to the start of the pattern
    pattern_start: Atomic<Position>,
    pub meter: Arc<Meter>,
}

impl Clone for MidiClip {
    fn clone(&self) -> Self {
        Self {
            pattern: self.pattern.clone(),
            global_start: Atomic::new(self.global_start.load(SeqCst)),
            global_end: Atomic::new(self.global_end.load(SeqCst)),
            pattern_start: Atomic::new(self.pattern_start.load(SeqCst)),
            meter: self.meter.clone(),
        }
    }
}

impl MidiClip {
    pub fn create(pattern: Arc<MidiPattern>, meter: Arc<Meter>) -> Arc<TrackClip> {
        let len = pattern.len();
        Arc::new(TrackClip::Midi(Self {
            pattern,
            global_start: Atomic::default(),
            global_end: Atomic::new(Position::from_interleaved_samples(len, &meter)),
            pattern_start: Atomic::default(),
            meter,
        }))
    }

    pub fn get_global_start(&self) -> Position {
        self.global_start.load(SeqCst)
    }

    pub fn get_global_end(&self) -> Position {
        self.global_end.load(SeqCst)
    }

    pub fn get_pattern_start(&self) -> Position {
        self.pattern_start.load(SeqCst)
    }

    pub fn trim_start_to(&self, global_start: Position) {
        let global_start = global_start.clamp(
            self.get_global_start()
                .saturating_sub(self.get_pattern_start()),
            self.get_global_end() - Position::SUB_QUARTER_NOTE,
        );
        let diff = self.get_global_start().abs_diff(global_start);
        if self.get_global_start() < global_start {
            self.pattern_start
                .fetch_update(SeqCst, SeqCst, |pattern_start| Some(pattern_start + diff))
                .unwrap();
        } else {
            self.pattern_start
                .fetch_update(SeqCst, SeqCst, |pattern_start| Some(pattern_start - diff))
                .unwrap();
        }
        self.global_start.store(global_start, SeqCst);
        self.pattern.dirty.store(DirtyEvent::NoteReplaced, SeqCst);
    }

    pub fn trim_end_to(&self, global_end: Position) {
        let global_end = global_end.max(self.get_global_start() + Position::SUB_QUARTER_NOTE);
        self.global_end.store(global_end, SeqCst);
        self.pattern.dirty.store(DirtyEvent::NoteReplaced, SeqCst);
    }

    pub fn move_to(&self, global_start: Position) {
        let diff = self.get_global_start().abs_diff(global_start);
        if self.get_global_start() < global_start {
            self.global_end
                .fetch_update(SeqCst, SeqCst, |global_end| Some(global_end + diff))
                .unwrap();
        } else {
            self.global_end
                .fetch_update(SeqCst, SeqCst, |global_end| Some(global_end - diff))
                .unwrap();
        }
        self.global_start.store(global_start, SeqCst);
        self.pattern.dirty.store(DirtyEvent::NoteReplaced, SeqCst);
    }
}
