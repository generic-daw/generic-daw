use crate::{MusicalTime, Transport};
use std::ops::Add;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Position {
	start: MusicalTime,
	end: MusicalTime,
}

impl Default for Position {
	fn default() -> Self {
		Self::new(MusicalTime::ZERO, MusicalTime::ZERO)
	}
}

impl Position {
	#[must_use]
	pub fn new(start: MusicalTime, end: MusicalTime) -> Self {
		Self {
			start,
			end: end.max(start + MusicalTime::TICK),
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
	pub const fn start(self) -> MusicalTime {
		self.start
	}

	#[must_use]
	pub const fn end(self) -> MusicalTime {
		self.end
	}

	#[must_use]
	pub fn len(self) -> MusicalTime {
		self.end() - self.start()
	}

	pub fn trim_start_to(&mut self, new_start: MusicalTime) {
		self.start = new_start.min(self.end - MusicalTime::TICK);
	}

	pub fn trim_end_to(&mut self, new_end: MusicalTime) {
		self.end = new_end.max(self.start + MusicalTime::TICK);
	}

	pub fn move_to(&mut self, new_start: MusicalTime) {
		let diff = self.start.abs_diff(new_start);
		if self.start < new_start {
			self.end += diff;
		} else {
			self.end -= diff;
		}
		self.start = new_start;
	}

	#[must_use]
	pub fn saturating_sub(mut self, diff: MusicalTime) -> Option<Self> {
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

	fn adjust(self, start: MusicalTime, end: MusicalTime, modulo: MusicalTime) -> Self {
		if start != end {
			Self::new(start, end)
		} else if self.start.abs_diff(start) < self.end.abs_diff(end) {
			Self::new(start, start + modulo)
		} else {
			Self::new(end - modulo, end)
		}
	}

	#[must_use]
	pub fn floor(self, modulo: MusicalTime) -> Self {
		self.adjust(self.start.floor(modulo), self.end.floor(modulo), modulo)
	}

	#[must_use]
	pub fn ceil(self, modulo: MusicalTime) -> Self {
		self.adjust(self.start.ceil(modulo), self.end.ceil(modulo), modulo)
	}

	#[must_use]
	pub fn round(self, modulo: MusicalTime) -> Self {
		self.adjust(self.start.round(modulo), self.end.round(modulo), modulo)
	}

	#[must_use]
	pub fn beat_floor(self) -> Self {
		self.floor(MusicalTime::BEAT)
	}

	#[must_use]
	pub fn beat_ceil(self) -> Self {
		self.ceil(MusicalTime::BEAT)
	}

	#[must_use]
	pub fn beat_round(self) -> Self {
		self.round(MusicalTime::BEAT)
	}

	#[must_use]
	pub fn bar_floor(self, transport: &Transport) -> Self {
		self.floor(MusicalTime::new(transport.numerator.get().into(), 0))
	}

	#[must_use]
	pub fn bar_ceil(self, transport: &Transport) -> Self {
		self.ceil(MusicalTime::new(transport.numerator.get().into(), 0))
	}

	#[must_use]
	pub fn bar_round(self, transport: &Transport) -> Self {
		self.round(MusicalTime::new(transport.numerator.get().into(), 0))
	}

	#[must_use]
	pub fn snap_floor(self, scale: f32, transport: &Transport) -> Self {
		self.floor(MusicalTime::snap_step(scale, transport))
	}

	#[must_use]
	pub fn snap_ceil(self, scale: f32, transport: &Transport) -> Self {
		self.ceil(MusicalTime::snap_step(scale, transport))
	}

	#[must_use]
	pub fn snap_round(self, scale: f32, transport: &Transport) -> Self {
		self.round(MusicalTime::snap_step(scale, transport))
	}
}

impl Add<MusicalTime> for Position {
	type Output = Self;

	fn add(mut self, rhs: MusicalTime) -> Self::Output {
		self.start += rhs;
		self.end += rhs;
		self
	}
}
