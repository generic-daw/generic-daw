use crate::generic_back::{AtomicDirtyEvent, DirtyEvent, MidiNote, MidiTrack};
use std::sync::{atomic::Ordering::SeqCst, Arc};

pub struct MidiPattern {
    pub notes: Vec<Arc<MidiNote>>,
    pub dirty: Arc<AtomicDirtyEvent>,
}

impl MidiPattern {
    pub fn new(track: &MidiTrack) -> Self {
        Self {
            notes: Vec::new(),
            dirty: track.plugin_state.read().unwrap().dirty.clone(),
        }
    }

    pub fn len(&self) -> u32 {
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
