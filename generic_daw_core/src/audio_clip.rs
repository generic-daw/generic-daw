use crate::{ClipPosition, MusicalTime, RtState};
use std::sync::Arc;

mod sample;

pub use sample::Sample;

#[derive(Clone, Debug)]
pub struct AudioClip {
	pub sample: Arc<Sample>,
	pub position: ClipPosition,
}

impl AudioClip {
	#[must_use]
	pub fn create(sample: Arc<Sample>, rtstate: &RtState) -> Arc<Self> {
		let len = sample.audio.len();

		Arc::new(Self {
			sample,
			position: ClipPosition::with_len(MusicalTime::from_samples(len, rtstate)),
		})
	}

	pub fn process(&self, rtstate: &RtState, audio: &mut [f32]) {
		if !rtstate.playing {
			return;
		}

		let start = self.position.start().to_samples(rtstate);
		let diff = rtstate.sample.abs_diff(start);

		if rtstate.sample > start {
			let start_index = diff + self.position.offset().to_samples(rtstate);

			if start_index >= self.sample.audio.len() {
				return;
			}

			self.sample.audio[start_index..]
				.iter()
				.zip(audio)
				.for_each(|(sample, buf)| {
					*buf += sample;
				});
		} else {
			if diff >= audio.len() {
				return;
			}

			self.sample
				.audio
				.iter()
				.zip(audio[diff..].iter_mut())
				.for_each(|(sample, buf)| {
					*buf += sample;
				});
		}
	}
}
