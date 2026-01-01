use crate::MusicalTime;
use std::ops::Add;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Position {
	start: MusicalTime,
	end: MusicalTime,
}

impl Default for Position {
	fn default() -> Self {
		Self::new(MusicalTime::ZERO, MusicalTime::TICK)
	}
}

impl Position {
	#[must_use]
	pub fn new(start: MusicalTime, end: MusicalTime) -> Self {
		debug_assert!(start < end);
		Self { start, end }
	}

	#[must_use]
	pub fn start(self) -> MusicalTime {
		self.start
	}

	#[must_use]
	pub fn end(self) -> MusicalTime {
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
