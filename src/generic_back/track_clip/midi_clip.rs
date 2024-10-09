use crate::generic_back::{DirtyEvent, Meter, Position, TrackClip};
use std::{
    cmp::Ordering,
    sync::{atomic::Ordering::SeqCst, Arc, RwLock},
};

mod midi_note;
pub use midi_note::MidiNote;

mod midi_pattern;
pub use midi_pattern::MidiPattern;

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

    pub fn trim_start_to(&self, global_start: Position) {
        let global_start = self
            .clamp(global_start)
            .min(*self.global_end.read().unwrap() - Position::MIN_STEP);
        let cmp = self.global_start.read().unwrap().cmp(&global_start);
        match cmp {
            Ordering::Less => {
                *self.pattern_start.write().unwrap() +=
                    global_start - *self.global_start.read().unwrap();
            }
            Ordering::Equal => {}
            Ordering::Greater => {
                *self.pattern_start.write().unwrap() -=
                    *self.global_start.read().unwrap() - global_start;
            }
        }
        *self.global_start.write().unwrap() = global_start;
        self.pattern.dirty.store(DirtyEvent::NoteReplaced, SeqCst);
    }

    pub fn trim_end_to(&self, global_end: Position) {
        let global_end = self
            .clamp(global_end)
            .max(*self.global_start.read().unwrap() + Position::MIN_STEP);
        *self.global_end.write().unwrap() = global_end;
        self.pattern.dirty.store(DirtyEvent::NoteReplaced, SeqCst);
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

    fn clamp(&self, position: Position) -> Position {
        position.clamp(
            self.global_start
                .read()
                .unwrap()
                .saturating_sub(*self.pattern_start.read().unwrap()),
            *self.global_start.read().unwrap()
                + Position::from_interleaved_samples(self.pattern.len(), &self.meter),
        )
    }

    pub(in crate::generic_back) fn get_global_midi(&self) -> Vec<Arc<MidiNote>> {
        let global_start = self
            .global_start
            .read()
            .unwrap()
            .in_interleaved_samples(&self.meter);
        let global_end = self
            .global_end
            .read()
            .unwrap()
            .in_interleaved_samples(&self.meter);
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
