use generic_daw_core::{self as core, MidiNote, MidiPatternId};

#[derive(Debug)]
pub struct MidiPattern {
	pub id: MidiPatternId,
	pub notes: Vec<MidiNote>,
	pub refs: usize,
}

#[derive(Debug)]
pub struct MidiPatternPair {
	pub core: core::MidiPattern,
	pub gui: MidiPattern,
}

impl MidiPatternPair {
	pub fn new(notes: Vec<MidiNote>) -> Self {
		let id = MidiPatternId::unique();
		Self {
			core: core::MidiPattern {
				id,
				notes: notes.clone(),
			},
			gui: MidiPattern { id, notes, refs: 0 },
		}
	}
}
