use std::f32::consts::TAU;

#[derive(Clone, Copy, Debug)]
pub struct BiquadCoeffs {
	a1: f32,
	a2: f32,
	b0: f32,
	b1: f32,
	b2: f32,
}

impl BiquadCoeffs {
	#[must_use]
	pub fn lowpass(sample_rate: f32, cutoff: f32, q: f32) -> Self {
		let omega = TAU * cutoff / sample_rate;
		let alpha = omega.sin() / (2.0 * q);
		let b0 = (1.0 - omega.cos()) / 2.0;
		let b1 = 1.0 - omega.cos();
		let b2 = (1.0 - omega.cos()) / 2.0;
		let a0 = 1.0 + alpha;
		let a1 = -2.0 * omega.cos();
		let a2 = 1.0 - alpha;
		Self {
			a1: a1 / a0,
			a2: a2 / a0,
			b0: b0 / a0,
			b1: b1 / a0,
			b2: b2 / a0,
		}
	}

	#[must_use]
	#[expect(clippy::manual_midpoint)]
	pub fn highpass(sample_rate: f32, cutoff: f32, q: f32) -> Self {
		let omega = TAU * cutoff / sample_rate;
		let alpha = omega.sin() / (2.0 * q);
		let b0 = (1.0 + omega.cos()) / 2.0;
		let b1 = -1.0 + omega.cos();
		let b2 = (1.0 + omega.cos()) / 2.0;
		let a0 = 1.0 + alpha;
		let a1 = -2.0 * omega.cos();
		let a2 = 1.0 - alpha;
		Self {
			a1: a1 / a0,
			a2: a2 / a0,
			b0: b0 / a0,
			b1: b1 / a0,
			b2: b2 / a0,
		}
	}

	#[must_use]
	pub fn bandpass(sample_rate: f32, center: f32, q: f32) -> Self {
		let omega = TAU * center / sample_rate;
		let alpha = omega.sin() / (2.0 * q);
		let b0 = alpha;
		let b1 = 0.0;
		let b2 = -alpha;
		let a0 = 1.0 + alpha;
		let a1 = -2.0 * omega.cos();
		let a2 = 1.0 - alpha;
		Self {
			a1: a1 / a0,
			a2: a2 / a0,
			b0: b0 / a0,
			b1: b1 / a0,
			b2: b2 / a0,
		}
	}

	#[must_use]
	pub fn notch(sample_rate: f32, center: f32, q: f32) -> Self {
		let omega = TAU * center / sample_rate;
		let alpha = omega.sin() / (2.0 * q);
		let b0 = 1.0;
		let b1 = -2.0 * omega.cos();
		let b2 = 1.0;
		let a0 = 1.0 + alpha;
		let a1 = -2.0 * omega.cos();
		let a2 = 1.0 - alpha;
		Self {
			a1: a1 / a0,
			a2: a2 / a0,
			b0: b0 / a0,
			b1: b1 / a0,
			b2: b2 / a0,
		}
	}

	#[must_use]
	pub fn allpass(sample_rate: f32, center: f32, q: f32) -> Self {
		let omega = TAU * center / sample_rate;
		let alpha = omega.sin() / (2.0 * q);
		let b0 = 1.0 - alpha;
		let b1 = -2.0 * omega.cos();
		let b2 = 1.0 + alpha;
		let a0 = 1.0 + alpha;
		let a1 = -2.0 * omega.cos();
		let a2 = 1.0 - alpha;
		Self {
			a1: a1 / a0,
			a2: a2 / a0,
			b0: b0 / a0,
			b1: b1 / a0,
			b2: b2 / a0,
		}
	}

	#[must_use]
	pub fn peaking(sample_rate: f32, center: f32, q: f32, gain: f32) -> Self {
		let omega = TAU * center / sample_rate;
		let alpha = omega.sin() / (2.0 * q);
		let a = gain.sqrt();
		let b0 = 1.0 + alpha * a;
		let b1 = -2.0 * omega.cos();
		let b2 = 1.0 - alpha * a;
		let a0 = 1.0 + alpha / a;
		let a1 = -2.0 * omega.cos();
		let a2 = 1.0 - alpha / a;
		Self {
			a1: a1 / a0,
			a2: a2 / a0,
			b0: b0 / a0,
			b1: b1 / a0,
			b2: b2 / a0,
		}
	}

	#[must_use]
	#[expect(clippy::suboptimal_flops)]
	pub fn lowshelf(sample_rate: f32, center: f32, q: f32, gain: f32) -> Self {
		let omega = TAU * center / sample_rate;
		let alpha = omega.sin() / (2.0 * q);
		let a = gain.sqrt();
		let b0 = a * ((a + 1.0) - (a - 1.0) * omega.cos() + 2.0 * a.sqrt() * alpha);
		let b1 = -2.0 * a * ((a - 1.0) - (a + 1.0) * omega.cos());
		let b2 = a * ((a + 1.0) - (a - 1.0) * omega.cos() - 2.0 * a.sqrt() * alpha);
		let a0 = (a + 1.0) + (a - 1.0) * omega.cos() + 2.0 * a.sqrt() * alpha;
		let a1 = 2.0 * ((a - 1.0) + (a + 1.0) * omega.cos());
		let a2 = (a + 1.0) + (a - 1.0) * omega.cos() - 2.0 * a.sqrt() * alpha;
		Self {
			a1: a1 / a0,
			a2: a2 / a0,
			b0: b0 / a0,
			b1: b1 / a0,
			b2: b2 / a0,
		}
	}

	#[must_use]
	#[expect(clippy::suboptimal_flops)]
	pub fn highshelf(sample_rate: f32, center: f32, q: f32, gain: f32) -> Self {
		let omega = TAU * center / sample_rate;
		let alpha = omega.sin() / (2.0 * q);
		let a = gain.sqrt();
		let b0 = a * ((a + 1.0) + (a - 1.0) * omega.cos() + 2.0 * a.sqrt() * alpha);
		let b1 = -2.0 * a * ((a - 1.0) + (a + 1.0) * omega.cos());
		let b2 = a * ((a + 1.0) + (a - 1.0) * omega.cos() - 2.0 * a.sqrt() * alpha);
		let a0 = (a + 1.0) - (a - 1.0) * omega.cos() + 2.0 * a.sqrt() * alpha;
		let a1 = 2.0 * ((a - 1.0) - (a + 1.0) * omega.cos());
		let a2 = (a + 1.0) - (a - 1.0) * omega.cos() - 2.0 * a.sqrt() * alpha;
		Self {
			a1: a1 / a0,
			a2: a2 / a0,
			b0: b0 / a0,
			b1: b1 / a0,
			b2: b2 / a0,
		}
	}
}

#[derive(Clone, Copy, Debug)]
pub struct Biquad {
	coeffs: BiquadCoeffs,
	x1: f32,
	x2: f32,
	y1: f32,
	y2: f32,
}

impl Biquad {
	#[must_use]
	pub fn new(coeffs: BiquadCoeffs) -> Self {
		Self {
			coeffs,
			x1: 0.0,
			x2: 0.0,
			y1: 0.0,
			y2: 0.0,
		}
	}

	#[must_use]
	pub fn tick(&mut self, x0: f32) -> f32 {
		let y0 = self.coeffs.b0.mul_add(
			x0,
			self.coeffs.b1.mul_add(
				self.x1,
				self.coeffs.b2.mul_add(
					self.x2,
					self.coeffs.a1.mul_add(self.y1, self.coeffs.a2 * self.y2),
				),
			),
		);
		self.x2 = self.x1;
		self.x1 = x0;
		self.y2 = self.y1;
		self.y1 = -y0;
		y0
	}

	pub fn process(&mut self, audio: &mut [f32]) {
		for sample in audio {
			*sample = self.tick(*sample);
		}
	}
}
