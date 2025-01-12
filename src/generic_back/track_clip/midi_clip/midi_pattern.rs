use crate::generic_back::{DirtyEvent, MidiNote, MidiTrack};
use atomig::Atomic;
use std::sync::{atomic::Ordering::SeqCst, Arc};

#[derive(Debug)]
pub struct MidiPattern {
    pub notes: Vec<Arc<MidiNote>>,
    pub(in crate::generic_back) dirty: Arc<Atomic<DirtyEvent>>,
}

impl MidiPattern {
    pub fn new(track: &MidiTrack) -> Self {
        Self {
            notes: Vec::new(),
            dirty: track.plugin_state.lock().unwrap().dirty.clone(),
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

    pub fn remove(&mut self, note: &MidiNote) {
        let pos = self.notes.iter().position(|n| &**n == note).unwrap();
        self.notes.remove(pos);
        self.dirty.store(DirtyEvent::NoteRemoved, SeqCst);
    }

    pub fn replace(&mut self, note: &MidiNote, new_note: Arc<MidiNote>) {
        let pos = self.notes.iter().position(|n| &**n == note).unwrap();
        self.notes[pos] = new_note;
        self.dirty.store(DirtyEvent::NoteReplaced, SeqCst);
    }
}
