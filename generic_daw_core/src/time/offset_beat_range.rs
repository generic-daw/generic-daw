use crate::{
	Transport,
	time::{BeatRange, BeatTime},
};

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct OffsetBeatRange {
	position: BeatRange,
	offset: BeatTime,
}

impl OffsetBeatRange {
	#[must_use]
	pub const fn new(position: BeatRange, offset: BeatTime) -> Self {
		Self { position, offset }
	}

	#[must_use]
	pub const fn to_samples(self, transport: &Transport) -> (usize, usize, usize) {
		(
			self.start().to_samples(transport),
			self.end().to_samples(transport),
			self.offset().to_samples(transport),
		)
	}

	#[must_use]
	pub const fn start(self) -> BeatTime {
		self.position.start()
	}

	#[must_use]
	pub const fn end(self) -> BeatTime {
		self.position.end()
	}

	#[must_use]
	pub const fn offset(self) -> BeatTime {
		self.offset
	}

	#[must_use]
	pub fn len(self) -> BeatTime {
		self.position.len()
	}

	pub fn trim_start_to(&mut self, new_start: BeatTime) {
		let old_start = self.start();
		let min_start = old_start.saturating_sub(self.offset());
		self.position.trim_start_to(new_start.max(min_start));
		let new_start = self.start();
		let diff = new_start.abs_diff(old_start);
		if old_start < new_start {
			self.offset += diff;
		} else {
			self.offset -= diff;
		}
	}

	pub fn trim_end_to(&mut self, new_end: BeatTime) {
		self.position.trim_end_to(new_end);
	}

	pub fn move_to(&mut self, new_start: BeatTime) {
		self.position.move_to(new_start);
	}

	#[must_use]
	pub const fn position(self) -> BeatRange {
		self.position
	}
}

impl From<BeatRange> for OffsetBeatRange {
	fn from(value: BeatRange) -> Self {
		Self::new(value, BeatTime::ZERO)
	}
}
