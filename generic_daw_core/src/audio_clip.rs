use crate::{OffsetPosition, SampleId, audio_processor::State};

#[derive(Clone, Copy, Debug)]
pub struct AudioClip {
	pub sample: SampleId,
	pub position: OffsetPosition,
}

impl AudioClip {
	pub fn process(&self, state: &State, audio: &mut [f32]) {
		debug_assert!(state.transport.playing);

		let sample = &state.samples[&self.sample];

		let (start, end, offset) = self.position.to_samples(&state.transport);
		let len = sample.len().saturating_sub(offset).min(end - start);
		let uidx = state.transport.sample.abs_diff(start).min(len);

		if state.transport.sample > start {
			sample.mix_into(offset + uidx, &mut audio[..len - uidx]);
		} else {
			sample.mix_into(offset, &mut audio[uidx..uidx + len]);
		}
	}
}
