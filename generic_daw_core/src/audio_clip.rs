use crate::{ClipPosition, SampleId, daw_ctx::State};

#[derive(Clone, Copy, Debug)]
pub struct AudioClip {
	pub sample: SampleId,
	pub position: ClipPosition,
}

impl AudioClip {
	pub fn process(&self, state: &State, audio: &mut [f32]) {
		debug_assert!(state.transport.playing);

		let sample = &state.samples[*self.sample];

		let start = self.position.start().to_samples(&state.transport);
		let end = self.position.end().to_samples(&state.transport);
		let offset = self.position.offset().to_samples(&state.transport);
		let len = sample.samples.len().saturating_sub(offset).min(end - start);

		let uidx = state.transport.sample.abs_diff(start).min(len);

		if state.transport.sample > start {
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
