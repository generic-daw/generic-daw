pub fn resample_cubic(
	samples: &[f32],
	resample_ratio: f64,
	offset: usize,
) -> impl Iterator<Item = (f32, f32)> {
	debug_assert!(samples.len().is_multiple_of(2));

	let mut frame = offset as f64 * resample_ratio;

	if resample_ratio.is_sign_negative() {
		frame += (samples.len() / 2) as f64;
	}

	std::iter::from_fn(move || {
		let fract = frame.fract() as f32;
		let idx = 2 * frame as usize;
		frame += resample_ratio;

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

		Some((
			interp_cubic(l0, l1, l2, l3, fract),
			interp_cubic(r0, r1, r2, r3, fract),
		))
	})
}

fn interp_cubic(s0: f32, s1: f32, s2: f32, s3: f32, fract: f32) -> f32 {
	let c0 = s1;
	let c1 = (-1.0 / 3.0) * s0 + (-1.0 / 2.0) * s1 + s2 + (-1.0 / 6.0) * s3;
	let c2 = (1.0 / 2.0) * (s0 + s2) - s1;
	let c3 = (1.0 / 6.0) * (s3 - s0) + (1.0 / 2.0) * (s1 - s2);
	((c3 * fract + c2) * fract + c1) * fract + c0
}
