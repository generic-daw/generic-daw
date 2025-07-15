use crate::RtState;
use atomig::{Atom, AtomInteger};
use std::{
	fmt::{Debug, Formatter},
	ops::{Add, AddAssign, Mul, Sub, SubAssign},
};

#[derive(Atom, AtomInteger, Clone, Copy, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MusicalTime(u32);

impl Debug for MusicalTime {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("MusicalTime")
			.field("beat", &self.beat())
			.field("tick", &self.tick())
			.finish()
	}
}

impl MusicalTime {
	pub const ZERO: Self = Self::new(0, 0);
	pub const BEAT: Self = Self::new(1, 0);
	pub const TICK: Self = Self::new(0, 1);

	#[must_use]
	pub const fn new(beat: u32, tick: u32) -> Self {
		debug_assert!(tick < 256);
		debug_assert!(beat <= u32::MAX >> 8);

		Self((beat << 8) | tick)
	}

	#[must_use]
	pub const fn bar(self, rtstate: &RtState) -> u32 {
		self.beat() / rtstate.numerator as u32
	}

	#[must_use]
	pub const fn beat(self) -> u32 {
		self.0 >> 8
	}

	#[must_use]
	pub const fn tick(self) -> u32 {
		self.0 & 0xff
	}

	#[must_use]
	pub const fn floor(mut self) -> Self {
		self.0 &= !0xff;
		self
	}

	#[must_use]
	pub const fn ceil(mut self) -> Self {
		if self.tick() != 0 {
			self.0 &= !0xff;
			self.0 += 1 << 8;
		}

		self
	}

	#[must_use]
	pub const fn round(mut self) -> Self {
		if self.0 & 0x80 != 0 {
			self.0 += 1 << 8;
		}

		self.0 &= !0xff;

		self
	}

	#[must_use]
	pub const fn from_samples_f(samples: f32, rtstate: &RtState) -> Self {
		let samples = samples as f64;
		let bpm = rtstate.bpm as f64;
		let sample_rate = rtstate.sample_rate as f64;

		let time = samples * (bpm * 32.0) / (sample_rate * 15.0);

		Self(time as u32)
	}

	#[must_use]
	pub const fn from_samples(samples: usize, rtstate: &RtState) -> Self {
		let samples = samples as u64;
		let bpm = rtstate.bpm as u64;
		let sample_rate = rtstate.sample_rate as u64;

		let time = samples * (bpm * 32) / (sample_rate * 15);

		Self(time as u32)
	}

	#[must_use]
	pub const fn to_samples_f(self, rtstate: &RtState) -> f32 {
		let beat = self.0 as f64;
		let bpm = rtstate.bpm as f64;
		let sample_rate = rtstate.sample_rate as f64;

		let samples = beat * (sample_rate * 15.0) / (bpm * 32.0);

		samples as f32
	}

	#[must_use]
	pub const fn to_samples(self, rtstate: &RtState) -> usize {
		let time = self.0 as u64;
		let bpm = rtstate.bpm as u64;
		let sample_rate = rtstate.sample_rate as u64;

		let samples = time * (sample_rate * 15) / (bpm * 32);

		samples as usize
	}

	#[must_use]
	pub fn snap_floor(mut self, scale: f32, rtstate: &RtState) -> Self {
		let snap_step = Self::snap_step(scale, rtstate).0;
		self.0 -= self.0 % snap_step;
		self
	}

	#[must_use]
	pub fn snap_ceil(mut self, scale: f32, rtstate: &RtState) -> Self {
		let snap_step = Self::snap_step(scale, rtstate).0;
		self.0 += snap_step - (self.0 % snap_step);
		self
	}

	#[must_use]
	pub fn snap_round(mut self, scale: f32, rtstate: &RtState) -> Self {
		let modulo = Self::snap_step(scale, rtstate).0;

		let diff = self.0 % modulo;

		if diff > modulo / 2 {
			self.0 += modulo - diff;
		} else {
			self.0 -= diff;
		}

		self
	}

	#[must_use]
	pub fn snap_step(mut scale: f32, rtstate: &RtState) -> Self {
		scale += f32::from(rtstate.bpm).log2() - 10.0;
		Self(if scale < 0.0 {
			1
		} else if scale < 9.0 {
			1 << scale.floor() as u8
		} else {
			u32::from(rtstate.numerator) << (scale.floor() as u8 - 1)
		})
	}

	#[must_use]
	pub const fn saturating_sub(self, other: Self) -> Self {
		Self(self.0.saturating_sub(other.0))
	}

	#[must_use]
	pub const fn abs_diff(self, other: Self) -> Self {
		Self(self.0.abs_diff(other.0))
	}
}

impl Add for MusicalTime {
	type Output = Self;

	fn add(self, rhs: Self) -> Self::Output {
		Self(self.0 + rhs.0)
	}
}

impl AddAssign for MusicalTime {
	fn add_assign(&mut self, rhs: Self) {
		self.0 += rhs.0;
	}
}

impl Sub for MusicalTime {
	type Output = Self;

	fn sub(self, rhs: Self) -> Self::Output {
		Self(self.0 - rhs.0)
	}
}

impl SubAssign for MusicalTime {
	fn sub_assign(&mut self, rhs: Self) {
		self.0 -= rhs.0;
	}
}

impl Mul<u32> for MusicalTime {
	type Output = Self;

	fn mul(mut self, rhs: u32) -> Self::Output {
		self.0 *= rhs;
		self
	}
}

impl From<u32> for MusicalTime {
	fn from(value: u32) -> Self {
		Self(value)
	}
}

impl From<MusicalTime> for u32 {
	fn from(value: MusicalTime) -> Self {
		value.0
	}
}
