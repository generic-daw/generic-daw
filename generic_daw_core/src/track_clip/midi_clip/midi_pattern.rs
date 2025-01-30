use crate::{DirtyEvent, MidiNote, Track};
use atomig::Atomic;
use std::sync::{atomic::Ordering::SeqCst, Arc};

#[derive(Debug)]
pub struct MidiPattern {
    pub notes: Vec<MidiNote>,
    pub(crate) dirty: Arc<Atomic<DirtyEvent>>,
}

impl MidiPattern {
    #[must_use]
    pub fn new(track: &Track) -> Self {
        Self {
            notes: Vec::new(),
            dirty: track.dirty(),
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.notes
            .iter()
            .map(|note| note.local_end)
            .max()
            .unwrap_or(0)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn push(&mut self, note: MidiNote) {
        self.notes.push(note);
        self.dirty.store(DirtyEvent::NoteAdded, SeqCst);
    }

    pub fn remove(&mut self, note: &MidiNote) {
        let pos = self.notes.iter().position(|n| n == note).unwrap();
        self.notes.swap_remove(pos);
        self.dirty.store(DirtyEvent::NoteRemoved, SeqCst);
    }

    pub fn replace(&mut self, note: &MidiNote, new_note: MidiNote) {
        let pos = self.notes.iter().position(|n| n == note).unwrap();
        self.notes[pos] = new_note;
        self.dirty.store(DirtyEvent::NoteReplaced, SeqCst);
    }
}
