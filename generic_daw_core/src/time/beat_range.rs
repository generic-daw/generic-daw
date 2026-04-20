use crate::{Transport, time::BeatTime};
use std::ops::Add;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BeatRange {
	start: BeatTime,
	end: BeatTime,
}

impl Default for BeatRange {
	fn default() -> Self {
		Self::new(BeatTime::ZERO, BeatTime::ZERO)
	}
}

impl BeatRange {
	#[must_use]
	pub fn new(start: BeatTime, end: BeatTime) -> Self {
		Self {
			start,
			end: end.max(start + BeatTime::TICK),
		}
	}

	#[must_use]
	pub const fn to_samples(self, transport: &Transport) -> (usize, usize) {
		(
			self.start().to_samples(transport),
			self.end().to_samples(transport),
		)
	}

	#[must_use]
	pub const fn start(self) -> BeatTime {
		self.start
	}

	#[must_use]
	pub const fn end(self) -> BeatTime {
		self.end
	}

	#[must_use]
	pub fn len(self) -> BeatTime {
		self.end() - self.start()
	}

	pub fn trim_start_to(&mut self, new_start: BeatTime) {
		self.start = new_start.min(self.end - BeatTime::TICK);
	}

	pub fn trim_end_to(&mut self, new_end: BeatTime) {
		self.end = new_end.max(self.start + BeatTime::TICK);
	}

	pub fn move_to(&mut self, new_start: BeatTime) {
		let len = self.len();
		self.start = new_start;
		self.end = self.start + len;
	}

	#[must_use]
	pub fn saturating_sub(mut self, diff: BeatTime) -> Option<Self> {
		self.start = self.start.saturating_sub(diff);
		self.end = self.end.saturating_sub(diff);
		(self.start != self.end).then_some(self)
	}

	#[must_use]
	pub fn clamp(mut self, other: Self) -> Option<Self> {
		self.start = self.start.clamp(other.start, other.end);
		self.end = self.end.clamp(other.start, other.end);
		(self.start != self.end).then_some(self)
	}

	fn adjust(self, start: BeatTime, end: BeatTime, modulo: BeatTime) -> Self {
		if start != end {
			Self::new(start, end)
		} else if self.start.abs_diff(start) < self.end.abs_diff(end) {
			Self::new(start, start + modulo)
		} else {
			Self::new(end - modulo, end)
		}
	}

	#[must_use]
	pub fn floor(self, modulo: BeatTime) -> Self {
		self.adjust(self.start.floor(modulo), self.end.floor(modulo), modulo)
	}

	#[must_use]
	pub fn ceil(self, modulo: BeatTime) -> Self {
		self.adjust(self.start.ceil(modulo), self.end.ceil(modulo), modulo)
	}

	#[must_use]
	pub fn round(self, modulo: BeatTime) -> Self {
		self.adjust(self.start.round(modulo), self.end.round(modulo), modulo)
	}

	#[must_use]
	pub fn beat_floor(self) -> Self {
		self.floor(BeatTime::BEAT)
	}

	#[must_use]
	pub fn beat_ceil(self) -> Self {
		self.ceil(BeatTime::BEAT)
	}

	#[must_use]
	pub fn beat_round(self) -> Self {
		self.round(BeatTime::BEAT)
	}

	#[must_use]
	pub fn bar_floor(self, transport: &Transport) -> Self {
		self.floor(BeatTime::new(transport.numerator.get().into(), 0))
	}

	#[must_use]
	pub fn bar_ceil(self, transport: &Transport) -> Self {
		self.ceil(BeatTime::new(transport.numerator.get().into(), 0))
	}

	#[must_use]
	pub fn bar_round(self, transport: &Transport) -> Self {
		self.round(BeatTime::new(transport.numerator.get().into(), 0))
	}

	#[must_use]
	pub fn stretch(self, stretch: f32) -> Self {
		Self::new(self.start(), self.start() + self.len() * stretch)
	}
}

impl Add<BeatTime> for BeatRange {
	type Output = Self;

	fn add(mut self, rhs: BeatTime) -> Self::Output {
		self.start += rhs;
		self.end += rhs;
		self
	}
}
