use crate::time::SecondsTime;
use dsp::{transition_asymmetric, transition_symmetric};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
	pub x: f32,
	pub y: f32,
}

impl Default for Point {
	fn default() -> Self {
		Self { x: 0.5, y: 0.5 }
	}
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Transition {
	pub len: SecondsTime,
	pub p: Point,
	pub symmetric: bool,
}

impl Transition {
	#[must_use]
	pub fn transition(&self, at: f32) -> f32 {
		if self.symmetric {
			transition_symmetric(at, self.p.x, self.p.y)
		} else {
			transition_asymmetric(at, self.p.x, self.p.y)
		}
	}
}
