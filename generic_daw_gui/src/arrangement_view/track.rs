use generic_daw_core::{Clip, MusicalTime, NodeId};

#[derive(Debug)]
pub struct Track {
	pub id: NodeId,
	pub clips: Vec<Clip>,
}

impl Track {
	pub fn new(id: NodeId) -> Self {
		Self {
			id,
			clips: Vec::new(),
		}
	}

	pub fn len(&self) -> MusicalTime {
		self.clips
			.iter()
			.map(|clip| clip.position().end())
			.max()
			.unwrap_or_default()
	}
}
