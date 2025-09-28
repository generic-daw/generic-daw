use crate::arrangement_view::sample::Sample;
use generic_daw_core::{ClipPosition, SampleId};

#[derive(Clone, Copy, Debug)]
pub struct AudioClip {
	pub sample: SampleId,
	pub position: ClipPosition,
}

impl AudioClip {
	pub fn new(sample: SampleId) -> Self {
		Self {
			sample,
			position: ClipPosition::default(),
		}
	}
}

#[derive(Clone, Copy, Debug)]
pub struct AudioClipRef<'a> {
	pub sample: &'a Sample,
	pub clip: &'a AudioClip,
}
