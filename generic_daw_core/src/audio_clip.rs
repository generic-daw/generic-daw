use crate::{ClipPosition, SampleId, daw_ctx::State};

#[derive(Clone, Copy, Debug)]
pub struct AudioClip {
	pub sample: SampleId,
	pub position: ClipPosition,
}

impl AudioClip {
	pub fn process(&self, state: &State, audio: &mut [f32]) {
		debug_assert!(state.rtstate.playing);

		let sample = &state.samples[*self.sample];

		let start = self.position.start().to_samples(&state.rtstate);
		let end = self.position.end().to_samples(&state.rtstate);
		let offset = self.position.offset().to_samples(&state.rtstate);
		let len = sample.samples.len().saturating_sub(offset).min(end - start);

		let uidx = state.rtstate.sample.abs_diff(start);

		if state.rtstate.sample > start {
			sample.samples[offset..][..len][uidx..]
				.iter()
				.zip(audio)
				.for_each(|(sample, buf)| *buf += sample);
		} else {
			sample.samples[offset..][..len]
				.iter()
				.zip(&mut audio[uidx..])
				.for_each(|(sample, buf)| *buf += sample);
		}
	}
}
