use crate::{
	Transport,
	time::{BeatTime, SecondsTime},
};
use std::ops::Add;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BeatSpan {
	start: BeatTime,
	len: SecondsTime,
}

impl Default for BeatSpan {
	fn default() -> Self {
		Self::new(BeatTime::ZERO, SecondsTime::ZERO)
	}
}

impl BeatSpan {
	#[must_use]
	pub fn new(start: BeatTime, len: SecondsTime) -> Self {
		Self {
			start,
			len: len.max(SecondsTime::TICK),
		}
	}

	#[must_use]
	pub fn to_samples(self, transport: &Transport) -> (usize, usize) {
		(
			self.start().to_samples(transport),
			self.end(transport).to_samples(transport),
		)
	}

	#[must_use]
	pub const fn start(self) -> BeatTime {
		self.start
	}

	#[must_use]
	pub fn end(self, transport: &Transport) -> BeatTime {
		self.start() + self.len().to_beat_time(transport)
	}

	#[must_use]
	pub const fn len(self) -> SecondsTime {
		self.len
	}

	pub fn trim_start_to(&mut self, new_start: BeatTime, transport: &Transport) {
		let end = self.end(transport);
		self.start = new_start.min(self.end(transport) - BeatTime::TICK);
		self.len = (end - self.start).to_seconds_time(transport);
	}

	pub fn trim_end_to(&mut self, new_end: BeatTime, transport: &Transport) {
		self.len = (new_end.saturating_sub(self.start()))
			.to_seconds_time(transport)
			.max(SecondsTime::TICK);
	}

	pub const fn move_to(&mut self, new_start: BeatTime) {
		self.start = new_start;
	}
}

impl Add<BeatTime> for BeatSpan {
	type Output = Self;

	fn add(mut self, rhs: BeatTime) -> Self::Output {
		self.start += rhs;
		self
	}
}
