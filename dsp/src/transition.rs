#[must_use]
pub fn transition_symmetric(p: f32, x: f32, y: f32) -> f32 {
	debug_assert!((0.0..=1.0).contains(&p));
	debug_assert!((0.0..=1.0).contains(&x));
	debug_assert!((0.0..=1.0).contains(&y));

	let p = 2.0 * p - 1.0;
	transition_asymmetric(p.abs(), y, x).copysign(p) / 2.0 + 0.5
}

#[must_use]
#[expect(clippy::many_single_char_names)]
pub fn transition_asymmetric(p: f32, x: f32, y: f32) -> f32 {
	debug_assert!((0.0..=1.0).contains(&p));
	debug_assert!((0.0..=1.0).contains(&x));
	debug_assert!((0.0..=1.0).contains(&y));

	let a = 1.0 - 2.0 * x;
	let b = 1.0 - 2.0 * y;

	let t = if a.abs() < f32::EPSILON {
		p
	} else {
		((x.powi(2) + a * p).sqrt() - x) / a
	};

	b * t * (t - 1.0) + t
}
