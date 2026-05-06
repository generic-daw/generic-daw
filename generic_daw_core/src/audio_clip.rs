use crate::{SampleId, audio_thread::State, time::OffsetBeatSpan};
use dsp::resample_cubic;
use utils::unique_id;

unique_id!(audio_clip_id);

pub use audio_clip_id::Id as AudioClipId;

#[derive(Clone, Copy, Debug)]
pub struct AudioClip {
	pub id: AudioClipId,
	pub sample: SampleId,
	pub position: OffsetBeatSpan,
	pub stretch: f64,
}

impl AudioClip {
	pub fn process(&self, state: &State, audio: &mut [f32]) {
		debug_assert!(state.transport.playing);

		let start = self.position.start().to_samples(&state.transport);

		let write_start =
			start.saturating_sub(state.transport.position.to_samples(&state.transport));

		if write_start >= audio.len() {
			return;
		}

		let play_pos = state
			.transport
			.position
			.to_samples(&state.transport)
			.saturating_sub(start);

		let sample = &state.samples[&self.sample];

		let resample_ratio = sample.resample_ratio(&state.transport);

		let offset = (self.position.offset() * resample_ratio).to_samples(&state.transport);

		let resample_ratio = self.stretch * resample_ratio;

		let read_len = sample
			.samples
			.len()
			.saturating_sub(offset)
			.min((self.position.len() * resample_ratio.abs()).to_samples(&state.transport));

		let read_start = if resample_ratio.is_sign_positive() {
			offset.min(sample.samples.len())
		} else {
			sample.samples.len().saturating_sub(read_len + offset)
		};

		resample_cubic(
			&mut audio[write_start..],
			&sample.samples[read_start..][..read_len],
			resample_ratio,
			play_pos / 2,
		);
	}
}
