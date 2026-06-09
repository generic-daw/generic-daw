use crate::{
	Transport,
	time::{BeatTime, fixed_point::FixedPoint},
};
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Sub, SubAssign};

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SecondsTime(FixedPoint);

impl SecondsTime {
	pub const ZERO: Self = Self::new(0, 0);
	pub const SECOND: Self = Self::new(1, 0);
	pub const TICK: Self = Self::new(0, 1);

	pub const FACTOR: u64 = FixedPoint::FACTOR;

	#[must_use]
	pub const fn new(second: u64, tick: u64) -> Self {
		Self(FixedPoint::new(second, tick))
	}

	#[must_use]
	pub const fn from_bits(bits: u64) -> Self {
		Self(FixedPoint::from_bits(bits))
	}

	#[must_use]
	pub const fn to_bits(self) -> u64 {
		self.0.to_bits()
	}

	#[must_use]
	pub const fn from_float(val: f64) -> Self {
		Self(FixedPoint::from_float(val))
	}

	#[must_use]
	pub const fn to_float(self) -> f64 {
		self.0.to_float()
	}

	#[must_use]
	pub const fn second(self) -> u64 {
		self.0.unit()
	}

	#[must_use]
	pub const fn tick(self) -> u64 {
		self.0.tick()
	}

	#[must_use]
	pub const fn from_frames(frames: usize, transport: &Transport) -> Self {
		let frames = frames as u64;
		let sample_rate = transport.sample_rate.get() as u64;

		let time = frames * Self::FACTOR / sample_rate;

		Self::from_bits(time)
	}

	#[must_use]
	pub const fn to_frames(self, transport: &Transport) -> usize {
		let time = self.to_bits();
		let sample_rate = transport.sample_rate.get() as u64;

		let frames = (time * sample_rate) / Self::FACTOR;

		frames as usize
	}

	#[must_use]
	pub fn to_clap(self) -> clap_host::SecondsTime {
		clap_host::SecondsTime::from_float(self.to_float())
	}

	#[must_use]
	pub const fn to_beat_time(self, transport: &Transport) -> BeatTime {
		BeatTime::from_bits(self.to_bits() * transport.bpm.get() as u64 / 60)
	}

	#[must_use]
	pub const fn checked_sub(self, other: Self) -> Option<Self> {
		if let Some(bits) = self.0.checked_sub(other.0) {
			Some(Self(bits))
		} else {
			None
		}
	}

	#[must_use]
	pub const fn saturating_sub(self, other: Self) -> Self {
		Self(self.0.saturating_sub(other.0))
	}

	#[must_use]
	pub const fn abs_diff(self, other: Self) -> Self {
		Self(self.0.abs_diff(other.0))
	}

	#[must_use]
	pub const fn floor(self, modulo: Self) -> Self {
		Self(self.0.floor(modulo.0))
	}

	#[must_use]
	pub const fn ceil(self, modulo: Self) -> Self {
		Self(self.0.ceil(modulo.0))
	}

	#[must_use]
	pub const fn round(self, modulo: Self) -> Self {
		Self(self.0.round(modulo.0))
	}

	#[must_use]
	pub const fn second_floor(self) -> Self {
		self.floor(Self::SECOND)
	}

	#[must_use]
	pub const fn second_ceil(self) -> Self {
		self.ceil(Self::SECOND)
	}

	#[must_use]
	pub const fn second_round(self) -> Self {
		self.round(Self::SECOND)
	}
}

impl Add for SecondsTime {
	type Output = Self;

	fn add(self, rhs: Self) -> Self::Output {
		Self(self.0 + rhs.0)
	}
}

impl AddAssign for SecondsTime {
	fn add_assign(&mut self, rhs: Self) {
		*self = *self + rhs;
	}
}

impl Sub for SecondsTime {
	type Output = Self;

	fn sub(self, rhs: Self) -> Self::Output {
		Self(self.0 - rhs.0)
	}
}

impl SubAssign for SecondsTime {
	fn sub_assign(&mut self, rhs: Self) {
		*self = *self - rhs;
	}
}

impl Mul<f64> for SecondsTime {
	type Output = Self;

	fn mul(self, rhs: f64) -> Self::Output {
		Self(self.0 * rhs)
	}
}

impl MulAssign<f64> for SecondsTime {
	fn mul_assign(&mut self, rhs: f64) {
		*self = *self * rhs;
	}
}

impl Div<f64> for SecondsTime {
	type Output = Self;

	fn div(self, rhs: f64) -> Self::Output {
		Self(self.0 / rhs)
	}
}

impl DivAssign<f64> for SecondsTime {
	fn div_assign(&mut self, rhs: f64) {
		*self = *self / rhs;
	}
}

impl Div for SecondsTime {
	type Output = f64;

	fn div(self, rhs: Self) -> Self::Output {
		self.0 / rhs.0
	}
}
