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
	pub const fn to_samples_f(self, transport: &Transport) -> (f32, f32) {
		(
			self.start().to_samples_f(transport),
			self.end().to_samples_f(transport),
		)
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
		self.start = self.start.clamp(other.start(), other.end());
		self.end = self.end.clamp(other.start(), other.end());
		(self.start != self.end).then_some(self)
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
