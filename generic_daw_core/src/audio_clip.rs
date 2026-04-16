use crate::{OffsetPosition, SampleId, audio_processor::State};

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

		let offset = self.position.offset().to_samples(&state.transport) / 2;
		let uidx = state.transport.sample.abs_diff(start).min(end - start) / 2;

		let write_start = if state.transport.sample > start {
			0
		} else {
			uidx
		};

		for ([l, r], frame) in audio[2 * write_start..]
			.as_chunks_mut()
			.0
			.iter_mut()
			.zip(write_start..)
		{
			let frame = (offset + uidx + frame) as f32 * self.stretch;
			let fract = frame.fract();
			let idx = 2 * frame as usize;

			let [l0, r0] = if idx < 2 {
				[0.0, 0.0]
			} else {
				[sample.samples[idx - 2], sample.samples[idx - 1]]
			};

			let [l1, r1] = if idx + 1 >= sample.samples.len() {
				[0.0, 0.0]
			} else {
				[sample.samples[idx], sample.samples[idx + 1]]
			};

			let [l2, r2] = if idx + 3 >= sample.samples.len() {
				[0.0, 0.0]
			} else {
				[sample.samples[idx + 2], sample.samples[idx + 3]]
			};

			let [l3, r3] = if idx + 5 >= sample.samples.len() {
				break;
			} else {
				[sample.samples[idx + 4], sample.samples[idx + 5]]
			};

			*l += interp_cub_herm(l0, l1, l2, l3, fract);
			*r += interp_cub_herm(r0, r1, r2, r3, fract);
		}
	}
}

fn interp_cub_herm(s0: f32, s1: f32, s2: f32, s3: f32, fract: f32) -> f32 {
	let c0 = s1;
	let c1 = 0.5 * (s2 - s0);
	let c2 = s0 - 2.5 * s1 + 2.0 * s2 - 0.5 * s3;
	let c3 = 0.5 * (s3 - s0) + 1.5 * (s1 - s2);
	((c3 * fract + c2) * fract + c1) * fract + c0
}
