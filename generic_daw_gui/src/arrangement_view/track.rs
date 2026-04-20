use crate::arrangement_view::clip::Clip;
use generic_daw_core::{NodeId, Transport, time::BeatTime};

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

	pub fn len(&self, transport: &Transport) -> BeatTime {
		self.clips
			.iter()
			.map(|clip| clip.end(transport))
			.max()
			.unwrap_or_default()
	}
}
