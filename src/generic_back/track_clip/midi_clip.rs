use crate::{
    generic_back::{meter::Meter, position::Position},
    generic_front::drawable::{Drawable, TimelinePosition, TimelineScale},
};
use atomic_enum::atomic_enum;
use iced::{widget::canvas::Frame, Theme};
use std::sync::{atomic::Ordering::SeqCst, Arc, RwLock};

#[derive(PartialEq)]
pub struct MidiNote {
    pub channel: u8,
    pub note: u16,
    /// between 0.0 and 1.0
    pub velocity: f64,
    pub local_start: u32,
    pub local_end: u32,
}

#[atomic_enum]
#[derive(PartialEq, Eq)]
pub enum DirtyEvent {
    // can we reasonably assume that only one of these will happen per sample?
    None,
    NoteAdded,
    NoteRemoved,
    NoteReplaced,
}

pub struct MidiPattern {
    pub notes: Vec<Arc<MidiNote>>,
    dirty: Arc<AtomicDirtyEvent>,
}

impl MidiPattern {
    const fn new(dirty: Arc<AtomicDirtyEvent>) -> Self {
        Self {
            notes: Vec::new(),
            dirty,
        }
    }

    fn len(&self) -> u32 {
        self.notes
            .iter()
            .map(|note| note.local_end)
            .max()
            .unwrap_or(0)
    }

    fn push(&mut self, note: Arc<MidiNote>) {
        self.notes.push(note);
        self.dirty.store(DirtyEvent::NoteAdded, SeqCst);
    }

    fn remove(&mut self, note: &Arc<MidiNote>) {
        let pos = self.notes.iter().position(|n| n == note).unwrap();
        self.notes.remove(pos);
        self.dirty.store(DirtyEvent::NoteRemoved, SeqCst);
    }

    fn replace(&mut self, note: &Arc<MidiNote>, new_note: Arc<MidiNote>) {
        let pos = self.notes.iter().position(|n| n == note).unwrap();
        self.notes[pos] = new_note;
        self.dirty.store(DirtyEvent::NoteReplaced, SeqCst);
    }
}

pub struct MidiClip {
    pub pattern: Arc<RwLock<MidiPattern>>,
    global_start: Position,
    global_end: Position,
    pattern_start: Position,
}

impl MidiClip {
    pub fn new(pattern: Arc<RwLock<MidiPattern>>, meter: &Meter) -> Self {
        let len = pattern.read().unwrap().len();
        Self {
            pattern,
            global_start: Position::new(0, 0),
            global_end: Position::from_interleaved_samples(len, meter),
            pattern_start: Position::new(0, 0),
        }
    }

    pub const fn get_global_start(&self) -> Position {
        self.global_start
    }

    pub const fn get_global_end(&self) -> Position {
        self.global_end
    }

    pub fn trim_start_to(&mut self, clip_start: Position) {
        self.pattern_start = clip_start;
    }

    pub fn trim_end_to(&mut self, global_end: Position) {
        self.global_end = global_end;
    }

    pub fn move_start_to(&mut self, global_start: Position) {
        match self.global_start.cmp(&global_start) {
            std::cmp::Ordering::Less => {
                self.global_end += global_start - self.global_start;
            }
            std::cmp::Ordering::Equal => {}
            std::cmp::Ordering::Greater => {
                self.global_end += self.global_start - global_start;
            }
        }
        self.global_start = global_start;
    }

    pub fn get_global_midi(&self, meter: &Meter) -> Vec<Arc<MidiNote>> {
        let global_start = self.global_start.in_interleaved_samples(meter);
        let global_end = self.global_end.in_interleaved_samples(meter);
        self.pattern
            .read()
            .unwrap()
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

impl Drawable for MidiClip {
    fn draw(
        &self,
        _frame: &mut Frame,
        _scale: TimelineScale,
        _position: &TimelinePosition,
        _meter: &Meter,
        _theme: &Theme,
    ) {
        unimplemented!()
    }
}
