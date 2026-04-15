use crate::arrangement_view::sample::Sample;
use generic_daw_core::{OffsetPosition, SampleId};

#[derive(Clone, Copy, Debug)]
pub struct AudioClip {
	pub sample: SampleId,
	pub position: OffsetPosition,
	pub stretch: f32,
}

impl AudioClip {
	pub fn new(sample: SampleId) -> Self {
		Self {
			sample,
			position: OffsetPosition::default(),
			stretch: 1.0,
		}
	}
}

#[derive(Clone, Copy, Debug)]
pub struct AudioClipRef<'a> {
	pub sample: &'a Sample,
	pub clip: &'a AudioClip,
	pub idx: (usize, usize),
}
