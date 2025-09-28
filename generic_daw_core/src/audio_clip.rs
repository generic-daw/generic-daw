use crate::{ClipPosition, SampleId, daw_ctx::State};

#[derive(Clone, Copy, Debug)]
pub struct AudioClip {
	pub sample: SampleId,
	pub position: ClipPosition,
}

impl AudioClip {
	#[must_use]
	pub fn new(sample: SampleId) -> Self {
		Self {
			sample,
			position: ClipPosition::default(),
		}
	}

	pub fn process(&self, state: &State, audio: &mut [f32]) {
		if !state.rtstate.playing {
			return;
		}

		let sample = &state.samples[*self.sample];

		let start = self.position.start().to_samples(&state.rtstate);
		let end = self.position.end().to_samples(&state.rtstate);
		let offset = self.position.offset().to_samples(&state.rtstate);
		let len = (sample.samples.len() - offset).min(end - start);

		let uidx = state.rtstate.sample.abs_diff(start);

		if state.rtstate.sample > start {
			if uidx >= len {
				return;
			}

			sample.samples[offset..][..len][uidx..]
				.iter()
				.zip(audio)
				.for_each(|(sample, buf)| *buf += sample);
		} else {
			if uidx >= audio.len() {
				return;
			}

			sample.samples[offset..][..len]
				.iter()
				.zip(&mut audio[uidx..])
				.for_each(|(sample, buf)| *buf += sample);
		}
	}
}
