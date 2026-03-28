use crate::arrangement_view::sample::Sample;
use generic_daw_core::{MusicalTime, OffsetPosition, SampleId};

#[derive(Clone, Copy, Debug)]
pub struct AudioClip {
	pub sample: SampleId,
	pub position: OffsetPosition,
	pub gain: f32,
	pub fade_in: MusicalTime,
	pub fade_out: MusicalTime,
	pub reversed: bool,
}

impl AudioClip {
	pub fn new(sample: SampleId) -> Self {
		Self {
			sample,
			position: OffsetPosition::default(),
			gain: 1.0,
			fade_in: MusicalTime::ZERO,
			fade_out: MusicalTime::ZERO,
			reversed: false,
		}
	}
}

#[derive(Clone, Copy, Debug)]
pub struct AudioClipRef<'a> {
	pub sample: &'a Sample,
	pub clip: &'a AudioClip,
	pub idx: (usize, usize),
}
