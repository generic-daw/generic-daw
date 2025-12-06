use crate::{MidiKey, MidiNote, MusicalTime};
use utils::unique_id;

unique_id!(midi_pattern_id);

pub use midi_pattern_id::Id as MidiPatternId;

#[derive(Clone, Copy, Debug)]
pub enum MidiPatternAction {
	Add(MidiNote, usize),
	Remove(usize),
	ChangeKey(usize, MidiKey),
	MoveTo(usize, MusicalTime),
	TrimStartTo(usize, MusicalTime),
	TrimEndTo(usize, MusicalTime),
}

#[derive(Debug)]
pub struct MidiPattern {
	pub id: MidiPatternId,
	pub notes: Vec<MidiNote>,
}

impl Default for MidiPattern {
	fn default() -> Self {
		Self {
			id: MidiPatternId::unique(),
			notes: Vec::new(),
		}
	}
}

impl MidiPattern {
	#[must_use]
	pub fn new(notes: Vec<MidiNote>) -> Self {
		Self {
			id: MidiPatternId::unique(),
			notes,
		}
	}

	pub fn apply(&mut self, action: MidiPatternAction) {
		match action {
			MidiPatternAction::Add(note, idx) => self.notes.insert(idx, note),
			MidiPatternAction::Remove(index) => _ = self.notes.remove(index),
			MidiPatternAction::ChangeKey(index, key) => self.notes[index].key = key,
			MidiPatternAction::MoveTo(index, pos) => self.notes[index].position.move_to(pos),
			MidiPatternAction::TrimStartTo(index, pos) => {
				self.notes[index].position.trim_start_to(pos);
			}
			MidiPatternAction::TrimEndTo(index, pos) => self.notes[index].position.trim_end_to(pos),
		}
	}
}
