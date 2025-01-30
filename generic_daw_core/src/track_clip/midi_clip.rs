use crate::{DirtyEvent, Meter, Position, TrackClip};
use atomig::Atomic;
use midi_pattern::MidiPattern;
use std::sync::{atomic::Ordering::SeqCst, Arc};

pub mod midi_note;
pub mod midi_pattern;

#[derive(Debug)]
pub struct MidiClip {
    pub pattern: Arc<MidiPattern>,
    /// the start of the clip relative to the start of the arrangement
    global_start: Atomic<Position>,
    /// the end of the clip relative to the start of the arrangement
    global_end: Atomic<Position>,
    /// the start of the clip relative to the start of the pattern
    clip_start: Atomic<Position>,
    /// information relating to the playback of the arrangement
    pub meter: Arc<Meter>,
}

impl Clone for MidiClip {
    fn clone(&self) -> Self {
        Self {
            pattern: self.pattern.clone(),
            global_start: Atomic::new(self.global_start.load(SeqCst)),
            global_end: Atomic::new(self.global_end.load(SeqCst)),
            clip_start: Atomic::new(self.clip_start.load(SeqCst)),
            meter: self.meter.clone(),
        }
    }
}

impl MidiClip {
    #[must_use]
    pub fn create(pattern: Arc<MidiPattern>, meter: Arc<Meter>) -> Arc<TrackClip> {
        let len = pattern.len();
        Arc::new(TrackClip::Midi(Self {
            pattern,
            global_start: Atomic::default(),
            global_end: Atomic::new(Position::from_interleaved_samples(len, &meter)),
            clip_start: Atomic::default(),
            meter,
        }))
    }

    #[must_use]
    pub fn get_global_start(&self) -> Position {
        self.global_start.load(SeqCst)
    }

    #[must_use]
    pub fn get_global_end(&self) -> Position {
        self.global_end.load(SeqCst)
    }

    #[must_use]
    pub fn get_clip_start(&self) -> Position {
        self.clip_start.load(SeqCst)
    }

    pub fn trim_start_to(&self, global_start: Position) {
        let global_start = global_start.clamp(
            self.get_global_start()
                .saturating_sub(self.get_clip_start()),
            self.get_global_end() - Position::SUB_QUARTER_NOTE,
        );
        let diff = self.get_global_start().abs_diff(global_start);
        if self.get_global_start() < global_start {
            self.clip_start
                .fetch_update(SeqCst, SeqCst, |pattern_start| Some(pattern_start + diff))
                .unwrap();
        } else {
            self.clip_start
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
