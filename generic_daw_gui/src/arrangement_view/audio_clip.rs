use crate::arrangement_view::sample::Sample;
use generic_daw_core::{AudioClipId, SampleId, Transition, time::OffsetBeatSpan};

#[derive(Clone, Copy, Debug)]
pub struct AudioClip {
	pub id: AudioClipId,
	pub sample: SampleId,
	pub position: OffsetBeatSpan,
	pub fade_start: Transition,
	pub fade_end: Transition,
	pub stretch: f64,
}

impl AudioClip {
	pub fn new(sample: SampleId) -> Self {
		Self {
			id: AudioClipId::unique(),
			sample,
			position: OffsetBeatSpan::default(),
			fade_start: Transition::default(),
			fade_end: Transition::default(),
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
