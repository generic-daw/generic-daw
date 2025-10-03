use crate::arrangement_view::sample::Sample;
use generic_daw_core::{ClipPosition, MusicalTime, NotePosition, RtState, SampleId};

#[derive(Clone, Copy, Debug)]
pub struct AudioClip {
	pub sample: SampleId,
	pub position: ClipPosition,
}

impl AudioClip {
	pub fn new(sample: SampleId, len: usize, rtstate: &RtState) -> Self {
		Self {
			sample,
			position: ClipPosition::new(
				NotePosition::new(MusicalTime::ZERO, MusicalTime::from_samples(len, rtstate)),
				MusicalTime::ZERO,
			),
		}
	}
}

#[derive(Clone, Copy, Debug)]
pub struct AudioClipRef<'a> {
	pub sample: &'a Sample,
	pub clip: &'a AudioClip,
}
