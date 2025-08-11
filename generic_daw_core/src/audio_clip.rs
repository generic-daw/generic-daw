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
		let end = self.position.end().to_samples(rtstate);
		let offset = self.position.offset().to_samples(rtstate);
		let len = end - start;

		let uidx = rtstate.sample.abs_diff(start);

		if rtstate.sample > start {
			if uidx >= len {
				return;
			}

			self.sample.audio[offset..][..len][uidx..]
				.iter()
				.zip(audio)
				.for_each(|(sample, buf)| *buf += sample);
		} else {
			if uidx >= audio.len() {
				return;
			}

			self.sample.audio[offset..][..len]
				.iter()
				.zip(&mut audio[uidx..])
				.for_each(|(sample, buf)| *buf += sample);
		}
	}
}
