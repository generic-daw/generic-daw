use crate::arrangement_view::sample::Sample;
use generic_daw_core::{MusicalTime, OffsetPosition, Position, SampleId, Transport};

#[derive(Clone, Copy, Debug)]
pub struct AudioClip {
	pub sample: SampleId,
	pub position: OffsetPosition,
}

impl AudioClip {
	pub fn new(sample: SampleId, len: usize, transport: &Transport) -> Self {
		Self {
			sample,
			position: Position::new(
				MusicalTime::ZERO,
				MusicalTime::from_samples(len, transport).max(MusicalTime::TICK),
			)
			.into(),
		}
	}
}

#[derive(Clone, Copy, Debug)]
pub struct AudioClipRef<'a> {
	pub sample: &'a Sample,
	pub clip: &'a AudioClip,
	pub idx: (usize, usize),
}
