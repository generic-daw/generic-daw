use crate::generic_back::{DirtyEvent, Meter, Position, TrackClip};
use std::sync::{atomic::Ordering::SeqCst, Arc, RwLock};

pub use midi_note::MidiNote;
pub use midi_pattern::MidiPattern;

mod midi_note;
mod midi_pattern;

#[derive(Debug)]
pub struct MidiClip {
    pub pattern: Arc<MidiPattern>,
    /// the start of the clip relative to the start of the arrangement
    global_start: RwLock<Position>,
    /// the end of the clip relative to the start of the arrangement
    global_end: RwLock<Position>,
    /// the start of the clip relative to the start of the pattern
    pattern_start: RwLock<Position>,
    pub meter: Arc<Meter>,
}

impl Clone for MidiClip {
    fn clone(&self) -> Self {
        Self {
            pattern: self.pattern.clone(),
            global_start: RwLock::new(*self.global_start.read().unwrap()),
            global_end: RwLock::new(*self.global_end.read().unwrap()),
            pattern_start: RwLock::new(*self.pattern_start.read().unwrap()),
            meter: self.meter.clone(),
        }
    }
}

impl MidiClip {
    pub fn create(pattern: Arc<MidiPattern>, meter: Arc<Meter>) -> Arc<TrackClip> {
        let len = pattern.len();
        Arc::new(TrackClip::Midi(Self {
            pattern,
            global_start: RwLock::default(),
            global_end: RwLock::new(Position::from_interleaved_samples(len, &meter)),
            pattern_start: RwLock::default(),
            meter,
        }))
    }

    pub fn get_global_start(&self) -> Position {
        *self.global_start.read().unwrap()
    }

    pub fn get_global_end(&self) -> Position {
        *self.global_end.read().unwrap()
    }

    pub fn get_pattern_start(&self) -> Position {
        *self.pattern_start.read().unwrap()
    }

    pub fn trim_start_to(&self, global_start: Position) {
        let global_start = global_start.clamp(
            self.get_global_start()
                .saturating_sub(self.get_pattern_start()),
            self.get_global_end() - Position::MIN_STEP,
        );
        let diff = self.get_global_start().abs_diff(global_start);
        if self.get_global_start() < global_start {
            *self.pattern_start.write().unwrap() += diff;
        } else {
            *self.pattern_start.write().unwrap() -= diff;
        }
        *self.global_start.write().unwrap() = global_start;
        self.pattern.dirty.store(DirtyEvent::NoteReplaced, SeqCst);
    }

    pub fn trim_end_to(&self, global_end: Position) {
        let global_end = global_end.max(self.get_global_start() + Position::MIN_STEP);
        *self.global_end.write().unwrap() = global_end;
        self.pattern.dirty.store(DirtyEvent::NoteReplaced, SeqCst);
    }

    pub fn move_to(&self, global_start: Position) {
        let diff = self.get_global_start().abs_diff(global_start);
        if self.get_global_start() < global_start {
            *self.global_end.write().unwrap() += diff;
        } else {
            *self.global_end.write().unwrap() -= diff;
        }
        *self.global_start.write().unwrap() = global_start;
        self.pattern.dirty.store(DirtyEvent::NoteReplaced, SeqCst);
    }
}
