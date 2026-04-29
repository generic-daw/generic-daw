use std::{
	fmt::{Debug, Formatter},
	ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Sub, SubAssign},
};

#[derive(Clone, Copy, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FixedPoint(u64);

impl Debug for FixedPoint {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("FixedPoint")
			.field("unit", &self.unit())
			.field("tick", &self.tick())
			.finish()
	}
}

impl FixedPoint {
	pub const FACTOR: u64 = 28_224_000;

	#[must_use]
	pub const fn new(unit: u64, tick: u64) -> Self {
		debug_assert!(tick <= Self::FACTOR);
		debug_assert!(unit <= u64::MAX / Self::FACTOR);

		Self(unit * Self::FACTOR + tick)
	}

	#[must_use]
	pub const fn from_bits(bits: u64) -> Self {
		Self(bits)
	}

	#[must_use]
	pub const fn to_bits(self) -> u64 {
		self.0
	}

	#[must_use]
	pub const fn from_float(val: f64) -> Self {
		Self((Self::FACTOR as f64 * val).round() as u64)
	}

	#[must_use]
	pub const fn to_float(self) -> f64 {
		self.0 as f64 / Self::FACTOR as f64
	}

	#[must_use]
	pub const fn unit(self) -> u64 {
		self.0 / Self::FACTOR
	}

	#[must_use]
	pub const fn tick(self) -> u64 {
		self.0 % Self::FACTOR
	}

	#[must_use]
	pub const fn floor(mut self, modulo: Self) -> Self {
		self.0 -= self.0 % modulo.0;
		self
	}

	#[must_use]
	pub const fn ceil(mut self, modulo: Self) -> Self {
		self.0 = self.0.next_multiple_of(modulo.0);
		self
	}

	#[must_use]
	pub const fn round(mut self, modulo: Self) -> Self {
		let diff = self.0 % modulo.0;
		if diff < modulo.0 / 2 {
			self.0 -= diff;
		} else {
			self.0 += modulo.0 - diff;
		}
		self
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
}

impl Add for FixedPoint {
	type Output = Self;

	fn add(self, rhs: Self) -> Self::Output {
		Self(self.0 + rhs.0)
	}
}

impl AddAssign for FixedPoint {
	fn add_assign(&mut self, rhs: Self) {
		*self = *self + rhs;
	}
}

impl Sub for FixedPoint {
	type Output = Self;

	fn sub(self, rhs: Self) -> Self::Output {
		Self(self.0 - rhs.0)
	}
}

impl SubAssign for FixedPoint {
	fn sub_assign(&mut self, rhs: Self) {
		*self = *self - rhs;
	}
}

impl Mul<f64> for FixedPoint {
	type Output = Self;

	fn mul(self, rhs: f64) -> Self::Output {
		Self((self.0 as f64 * rhs) as u64)
	}
}

impl MulAssign<f64> for FixedPoint {
	fn mul_assign(&mut self, rhs: f64) {
		*self = *self * rhs;
	}
}

impl Div<f64> for FixedPoint {
	type Output = Self;

	fn div(self, rhs: f64) -> Self::Output {
		Self((self.0 as f64 / rhs) as u64)
	}
}

impl DivAssign<f64> for FixedPoint {
	fn div_assign(&mut self, rhs: f64) {
		*self = *self / rhs;
	}
}

impl Div for FixedPoint {
	type Output = f64;

	fn div(self, rhs: Self) -> Self::Output {
		self.0 as f64 / rhs.0 as f64
	}
}
