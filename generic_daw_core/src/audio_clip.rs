use crate::{OffsetPosition, SampleId, audio_processor::State};
use dsp::resample_cubic;

#[derive(Clone, Copy, Debug)]
pub struct AudioClip {
	pub sample: SampleId,
	pub position: OffsetPosition,
	pub stretch: f32,
}

impl AudioClip {
	pub fn process(&self, state: &State, audio: &mut [f32]) {
		debug_assert!(state.transport.playing);

		let (start, end) = self.position.position().to_samples(&state.transport);
		if !(start < state.transport.sample + audio.len() && end >= state.transport.sample) {
			return;
		}

		let sample = &state.samples[&self.sample];

		let uidx = state.transport.sample.abs_diff(start).min(end - start);
		let write_start = if state.transport.sample > start {
			0
		} else {
			uidx
		};

		let resample_ratio = sample.resample_ratio(&state.transport) * self.stretch;

		let (r_start, r_end, read_start) = self
			.position
			.stretch(resample_ratio)
			.to_samples(&state.transport);

		let read_len = sample
			.samples
			.len()
			.saturating_sub(read_start)
			.min(r_end - r_start);

		resample_cubic(
			&mut audio[write_start..],
			&sample.samples[read_start..][..read_len],
			resample_ratio,
			uidx / 2,
		);
	}
}
