use generic_daw_core::{self as core, MidiNote, PatternId};

#[derive(Debug)]
pub struct Pattern {
	pub id: PatternId,
	pub notes: Vec<MidiNote>,
}

pub struct PatternPair {
	pub core: core::Pattern,
	pub gui: Pattern,
}

impl PatternPair {
	pub fn new(notes: Vec<MidiNote>) -> Self {
		let id = PatternId::unique();
		Self {
			core: core::Pattern {
				id,
				notes: notes.clone(),
			},
			gui: Pattern { id, notes },
		}
	}
}
