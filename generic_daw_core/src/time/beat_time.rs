use crate::{
	Transport,
	time::{SecondsTime, fixed_point::FixedPoint},
};
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Sub, SubAssign};

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BeatTime(FixedPoint);

impl BeatTime {
	pub const ZERO: Self = Self::new(0, 0);
	pub const BEAT: Self = Self::new(1, 0);
	pub const TICK: Self = Self::new(0, 1);

	pub const FACTOR: u64 = FixedPoint::FACTOR;

	#[must_use]
	pub const fn new(beat: u64, tick: u64) -> Self {
		Self(FixedPoint::new(beat, tick))
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
	pub const fn bar(self, transport: &Transport) -> u64 {
		self.beat() / transport.numerator.get() as u64
	}

	#[must_use]
	pub const fn beat(self) -> u64 {
		self.0.unit()
	}

	#[must_use]
	pub const fn beat_in_bar(self, transport: &Transport) -> u64 {
		self.beat() % transport.numerator.get() as u64
	}

	#[must_use]
	pub const fn tick(self) -> u64 {
		self.0.tick()
	}

	#[must_use]
	pub const fn from_frames(frames: usize, transport: &Transport) -> Self {
		SecondsTime::from_frames(frames, transport).to_beat_time(transport)
	}

	#[must_use]
	pub const fn to_frames(self, transport: &Transport) -> usize {
		self.to_seconds_time(transport).to_frames(transport)
	}

	#[must_use]
	pub fn to_clap(self) -> clap_host::BeatTime {
		clap_host::BeatTime::from_float(self.to_float())
	}

	#[must_use]
	pub const fn to_seconds_time(self, transport: &Transport) -> SecondsTime {
		SecondsTime::from_bits((self.to_bits() * 60).div_ceil(transport.bpm.get() as u64))
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
	pub const fn beat_floor(self) -> Self {
		self.floor(Self::BEAT)
	}

	#[must_use]
	pub const fn beat_ceil(self) -> Self {
		self.ceil(Self::BEAT)
	}

	#[must_use]
	pub const fn beat_round(self) -> Self {
		self.round(Self::BEAT)
	}

	#[must_use]
	pub const fn bar_floor(self, transport: &Transport) -> Self {
		self.floor(Self::new(transport.numerator.get() as u64, 0))
	}

	#[must_use]
	pub const fn bar_ceil(self, transport: &Transport) -> Self {
		self.ceil(Self::new(transport.numerator.get() as u64, 0))
	}

	#[must_use]
	pub const fn bar_round(self, transport: &Transport) -> Self {
		self.round(Self::new(transport.numerator.get() as u64, 0))
	}
}

impl Add for BeatTime {
	type Output = Self;

	fn add(self, rhs: Self) -> Self::Output {
		Self(self.0 + rhs.0)
	}
}

impl AddAssign for BeatTime {
	fn add_assign(&mut self, rhs: Self) {
		*self = *self + rhs;
	}
}

impl Sub for BeatTime {
	type Output = Self;

	fn sub(self, rhs: Self) -> Self::Output {
		Self(self.0 - rhs.0)
	}
}

impl SubAssign for BeatTime {
	fn sub_assign(&mut self, rhs: Self) {
		*self = *self - rhs;
	}
}

impl Mul<f64> for BeatTime {
	type Output = Self;

	fn mul(self, rhs: f64) -> Self::Output {
		Self(self.0 * rhs)
	}
}

impl MulAssign<f64> for BeatTime {
	fn mul_assign(&mut self, rhs: f64) {
		*self = *self * rhs;
	}
}

impl Div<f64> for BeatTime {
	type Output = Self;

	fn div(self, rhs: f64) -> Self::Output {
		Self(self.0 / rhs)
	}
}

impl DivAssign<f64> for BeatTime {
	fn div_assign(&mut self, rhs: f64) {
		*self = *self / rhs;
	}
}

impl Div for BeatTime {
	type Output = f64;

	fn div(self, rhs: Self) -> Self::Output {
		self.0 / rhs.0
	}
}
