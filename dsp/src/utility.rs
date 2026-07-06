use std::f32::consts::{FRAC_PI_4, SQRT_2};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PanMode {
	Stereo(f32),
	SplitStereo(f32, f32),
}

#[derive(Clone, Copy, Debug)]
pub struct Utility {
	pub volume: f32,
	pub pan: PanMode,
}

impl Utility {
	pub fn process(&self, audio: &mut [[f32; 2]]) {
		fn split(pan: f32, fac: f32) -> (f32, f32) {
			let angle = (pan + 1.0) * FRAC_PI_4;
			let (sin, cos) = angle.sin_cos();
			(cos * fac, sin * fac)
		}

		match self.pan {
			PanMode::Stereo(pan) => {
				let (l, r) = split(pan, self.volume * SQRT_2);
				for [ls, rs] in audio {
					*ls *= l;
					*rs *= r;
				}
			}
			PanMode::SplitStereo(l, r) => {
				let (ll, lr) = split(l, self.volume);
				let (rl, rr) = split(r, self.volume);
				for [ls, rs] in audio {
					let ols = *ls;
					*ls = *ls * ll + *rs * rl;
					*rs = *rs * rr + ols * lr;
				}
			}
		}
	}
}
