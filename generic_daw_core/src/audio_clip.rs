use crate::{MusicalTime, OffsetPosition, SampleId, audio_processor::State};

#[derive(Clone, Copy, Debug)]
pub struct AudioClip {
	pub sample: SampleId,
	pub position: OffsetPosition,
	pub gain: f32,
	pub fade_in: MusicalTime,
	pub fade_out: MusicalTime,
	pub reversed: bool,
}

impl AudioClip {
	pub fn process(&self, state: &State, audio: &mut [f32]) {
		debug_assert!(state.transport.playing);

		let sample = &state.samples[&self.sample];

		let (start, end, offset) = self.position.to_samples(&state.transport);
		let len = sample.len().saturating_sub(offset).min(end - start);
		if len < 2 {
			return;
		}

		let clip_frames = len / 2;
		let fade_in_frames = self.fade_in.to_samples(&state.transport).min(len) / 2;
		let fade_out_frames = self.fade_out.to_samples(&state.transport).min(len) / 2;

		let clip_sample_offset = state.transport.sample.saturating_sub(start).min(len) / 2;
		let dst_offset = start.saturating_sub(state.transport.sample).min(audio.len()) / 2;
		let frames_to_process = clip_frames
			.saturating_sub(clip_sample_offset)
			.min(audio.len().saturating_sub(2 * dst_offset) / 2);

		sample.prefetch(offset, len);

		for frame in 0..frames_to_process {
			let clip_frame = clip_sample_offset + frame;
			let src_frame = if self.reversed {
				offset / 2 + clip_frames - clip_frame - 1
			} else {
				offset / 2 + clip_frame
			};
			let dst_frame = dst_offset + frame;

			let fade_in = if fade_in_frames == 0 {
				1.0
			} else {
				(clip_frame as f32 / fade_in_frames as f32).clamp(0.0, 1.0)
			};
			let fade_out = if fade_out_frames == 0 {
				1.0
			} else {
				((clip_frames - clip_frame) as f32 / fade_out_frames as f32).clamp(0.0, 1.0)
			};
			let gain = self.gain * fade_in.min(fade_out);

			let src = src_frame * 2;
			let dst = dst_frame * 2;

			if let Some(left) = sample.sample_at(src) {
				audio[dst] += left * gain;
			}
			if let Some(right) = sample.sample_at(src + 1) {
				audio[dst + 1] += right * gain;
			}
		}
	}
}
