use super::MidiKey;
use crate::MusicalTime;
use std::ops::Add;

#[derive(Clone, Copy, Debug)]
pub struct MidiNote {
	pub channel: u8,
	pub key: MidiKey,
	pub velocity: f64,
	pub start: MusicalTime,
	pub end: MusicalTime,
}

impl MidiNote {
	#[must_use]
	pub fn clamp(mut self, min: MusicalTime, max: MusicalTime) -> Option<Self> {
		if self.start > max || self.end < min {
			return None;
		}

		self.start = self.start.max(min);
		self.end = self.end.min(max);

		Some(self)
	}

	#[must_use]
	pub fn saturating_sub(mut self, other: MusicalTime) -> Option<Self> {
		if self.end < other {
			return None;
		}

		self.start = self.start.saturating_sub(other);
		self.end -= other;

		Some(self)
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
}

impl Add<MusicalTime> for MidiNote {
	type Output = Self;

	fn add(mut self, rhs: MusicalTime) -> Self::Output {
		self.start += rhs;
		self.end += rhs;

		self
	}
}
