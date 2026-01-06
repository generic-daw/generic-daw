use generic_daw_core::{self as core, MidiNote, MidiPatternId, MusicalTime, Transport};
use std::{path::Path, sync::Arc};

#[derive(Debug)]
pub struct MidiPattern {
	pub id: MidiPatternId,
	pub name: Arc<str>,
	pub notes: Vec<MidiNote>,
	pub refs: usize,
}

impl MidiPattern {
	pub fn len(&self) -> MusicalTime {
		self.notes
			.iter()
			.map(|note| note.position.end())
			.max()
			.unwrap_or_default()
	}
}

#[derive(Debug)]
pub struct MidiPatternPair {
	pub core: core::MidiPattern,
	pub gui: MidiPattern,
}

impl MidiPatternPair {
	pub fn from_notes(notes: Vec<MidiNote>, name: &str) -> Self {
		let core = core::MidiPattern::from_notes(notes);
		let gui = MidiPattern {
			id: core.id,
			name: name.into(),
			notes: core.notes.clone(),
			refs: 0,
		};
		Self { core, gui }
	}

	pub fn from_midi(path: Arc<Path>, transport: &Transport) -> Option<Self> {
		let name = path.file_name()?.to_str()?.into();
		let core = core::MidiPattern::from_midi(&std::fs::read(path).ok()?, transport)?;
		let gui = MidiPattern {
			id: core.id,
			name,
			notes: core.notes.clone(),
			refs: 0,
		};
		Some(Self { core, gui })
	}
}
