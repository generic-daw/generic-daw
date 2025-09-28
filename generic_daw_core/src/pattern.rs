use crate::{MidiNote, daw_ctx::PatternAction};
use generic_daw_utils::unique_id;

unique_id!(pattern_id);

pub use pattern_id::Id as PatternId;

#[derive(Debug)]
pub struct Pattern {
	pub id: PatternId,
	pub notes: Vec<MidiNote>,
}

impl Default for Pattern {
	fn default() -> Self {
		Self {
			id: PatternId::unique(),
			notes: Vec::new(),
		}
	}
}

impl Pattern {
	#[must_use]
	pub fn new(notes: Vec<MidiNote>) -> Self {
		Self {
			id: PatternId::unique(),
			notes,
		}
	}

	pub fn apply(&mut self, action: PatternAction) {
		match action {
			PatternAction::Add(note) => self.notes.push(note),
			PatternAction::Remove(index) => _ = self.notes.remove(index),
			PatternAction::ChangeKey(index, key) => self.notes[index].key = key,
			PatternAction::MoveTo(index, pos) => self.notes[index].position.move_to(pos),
			PatternAction::TrimStartTo(index, pos) => self.notes[index].position.trim_start_to(pos),
			PatternAction::TrimEndTo(index, pos) => self.notes[index].position.trim_end_to(pos),
		}
	}
}
