use crate::arrangement_view::sample::Sample;
use generic_daw_core::{OffsetPosition, SampleId};

#[derive(Clone, Copy, Debug)]
pub struct AudioClip {
	pub sample: SampleId,
	pub position: OffsetPosition,
}

impl AudioClip {
	pub fn new(sample: SampleId) -> Self {
		Self {
			sample,
			position: OffsetPosition::default(),
		}
	}
}

#[derive(Clone, Copy, Debug)]
pub struct AudioClipRef<'a> {
	pub sample: &'a Sample,
	pub clip: &'a AudioClip,
	pub idx: (usize, usize),
}
