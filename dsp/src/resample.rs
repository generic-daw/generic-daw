pub fn resample_cubic(audio: &mut [f32], samples: &[f32], resample_ratio: f32, offset: usize) {
	debug_assert!(audio.len().is_multiple_of(2));
	debug_assert!(samples.len().is_multiple_of(2));

	let resample_ratio = f64::from(resample_ratio);
	let offset = offset as f64 * resample_ratio;

	for (frame, [l, r]) in audio.as_chunks_mut().0.iter_mut().enumerate() {
		let frame = offset + frame as f64 * resample_ratio;
		let fract = frame.fract() as f32;
		let idx = 2 * frame as usize;

		if idx > samples.len() {
			break;
		}

		let [l0, r0] = if idx < 2 {
			[0.0, 0.0]
		} else {
			[samples[idx - 2], samples[idx - 1]]
		};

		let [l1, r1] = if idx + 1 >= samples.len() {
			[0.0, 0.0]
		} else {
			[samples[idx], samples[idx + 1]]
		};

		let [l2, r2] = if idx + 3 >= samples.len() {
			[0.0, 0.0]
		} else {
			[samples[idx + 2], samples[idx + 3]]
		};

		let [l3, r3] = if idx + 5 >= samples.len() {
			[0.0, 0.0]
		} else {
			[samples[idx + 4], samples[idx + 5]]
		};

		*l += interp_cubic(l0, l1, l2, l3, fract);
		*r += interp_cubic(r0, r1, r2, r3, fract);
	}
}

fn interp_cubic(s0: f32, s1: f32, s2: f32, s3: f32, fract: f32) -> f32 {
	let c0 = s1;
	let c1 = (-2.0 * s0 - 3.0 * s1 + 6.0 * s2 - s3) * (1.0 / 6.0);
	let c2 = (s0 - 2.0 * s1 + s2) * (1.0 / 2.0);
	let c3 = (-s0 + 3.0 * s1 - 3.0 * s2 + s3) * (1.0 / 6.0);
	((c3 * fract + c2) * fract + c1) * fract + c0
}
