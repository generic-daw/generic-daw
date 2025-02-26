use crate::{MidiNote, Position};

#[derive(Debug, Default)]
pub struct MidiPattern {
    notes: Vec<MidiNote>,
}

impl MidiPattern {
    #[must_use]
    pub fn len(&self) -> Position {
        self.notes
            .iter()
            .map(|note| note.end)
            .max()
            .unwrap_or_default()
    }

    #[must_use]
    pub fn notes(&self) -> &[MidiNote] {
        &self.notes
    }

    pub fn push(&mut self, note: MidiNote) {
        self.notes.push(note);
    }

    pub fn remove(&mut self, note: &MidiNote) {
        if let Some(pos) = self.notes.iter().position(|n| n == note) {
            self.notes.swap_remove(pos);
        }
    }

    pub fn replace(&mut self, note: &MidiNote, new_note: MidiNote) {
        if let Some(pos) = self.notes.iter().position(|n| n == note) {
            self.notes[pos] = new_note;
        }
    }
}
