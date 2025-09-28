use super::MidiKey;
use crate::{MusicalTime, NotePosition};
use std::ops::{Add, Sub};

#[derive(Clone, Copy, Debug)]
pub struct MidiNote {
	pub key: MidiKey,
	pub velocity: f32,
	pub position: NotePosition,
}

// impl MidiNote {
// 	#[must_use]
// 	pub fn clamp(mut self, min: MusicalTime, max: MusicalTime) -> Option<Self> {
// 		if self.start > max || self.end < min {
// 			return None;
// 		}

// 		self.start = self.start.max(min);
// 		self.end = self.end.min(max);

// 		Some(self)
// 	}

// 	#[must_use]
// 	pub fn saturating_sub(mut self, other: MusicalTime) -> Option<Self> {
// 		if self.end < other {
// 			return None;
// 		}

// 		self.start = self.start.saturating_sub(other);
// 		self.end -= other;

// 		Some(self)
// 	}
// }

impl Add<MusicalTime> for MidiNote {
	type Output = Self;

	fn add(mut self, rhs: MusicalTime) -> Self::Output {
		self.position.move_to(self.position.start() + rhs);
		self
	}
}

impl Sub<MusicalTime> for MidiNote {
	type Output = Self;

	fn sub(mut self, rhs: MusicalTime) -> Self::Output {
		self.position.move_to(self.position.start() - rhs);
		self
	}
}
