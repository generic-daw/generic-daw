use atomic_enum::atomic_enum;
use std::sync::{atomic::Ordering::SeqCst, Arc};

#[derive(PartialEq)]
pub struct MidiNote {
    pub channel: u8,
    pub note: u16,
    /// between 0.0 and 1.0
    pub velocity: f64,
    pub local_start: usize,
    pub local_end: usize,
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
    pub const fn new(dirty: Arc<AtomicDirtyEvent>) -> Self {
        Self {
            notes: Vec::new(),
            dirty,
        }
    }

    pub fn len(&self) -> usize {
        self.notes
            .iter()
            .map(|note| note.local_end)
            .max()
            .unwrap_or(0)
    }

    pub fn push(&mut self, note: Arc<MidiNote>) {
        self.notes.push(note);
        self.dirty.store(DirtyEvent::NoteAdded, SeqCst);
    }

    pub fn remove(&mut self, note: &Arc<MidiNote>) {
        let pos = self.notes.iter().position(|n| n == note).unwrap();
        self.notes.remove(pos);
        self.dirty.store(DirtyEvent::NoteRemoved, SeqCst);
    }

    pub fn replace(&mut self, note: &Arc<MidiNote>, new_note: Arc<MidiNote>) {
        let pos = self.notes.iter().position(|n| n == note).unwrap();
        self.notes[pos] = new_note;
        self.dirty.store(DirtyEvent::NoteReplaced, SeqCst);
    }
}
