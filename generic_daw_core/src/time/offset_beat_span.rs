use crate::{
	Transport,
	time::{BeatTime, SecondsTime, beat_span::BeatSpan},
};

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct OffsetBeatSpan {
	position: BeatSpan,
	offset: SecondsTime,
}

impl OffsetBeatSpan {
	#[must_use]
	pub const fn new(position: BeatSpan, offset: SecondsTime) -> Self {
		Self { position, offset }
	}

	#[must_use]
	pub fn to_samples(self, transport: &Transport) -> (usize, usize, usize) {
		(
			self.start().to_samples(transport),
			self.end(transport).to_samples(transport),
			self.offset().to_samples(transport),
		)
	}

	#[must_use]
	pub const fn start(self) -> BeatTime {
		self.position.start()
	}

	#[must_use]
	pub fn end(self, transport: &Transport) -> BeatTime {
		self.position.end(transport)
	}

	#[must_use]
	pub const fn offset(self) -> SecondsTime {
		self.offset
	}

	#[must_use]
	pub const fn len(self) -> SecondsTime {
		self.position.len()
	}

	pub fn trim_start_to(&mut self, new_start: BeatTime, transport: &Transport, stretch: f32) {
		let old_start = self.start();
		let min_start = old_start.saturating_sub(self.offset().to_beat_time(transport) / stretch);
		self.position
			.trim_start_to(new_start.max(min_start), transport);
		let new_start = self.start();
		let diff = new_start.abs_diff(old_start).to_seconds_time(transport) * stretch;
		if old_start < new_start {
			self.offset += diff;
		} else {
			self.offset -= diff;
		}
	}

	pub fn trim_end_to(&mut self, new_end: BeatTime, transport: &Transport) {
		self.position.trim_end_to(new_end, transport);
	}

	pub const fn move_to(&mut self, new_start: BeatTime) {
		self.position.move_to(new_start);
	}

	pub fn stretch_start_to(&mut self, new_start: BeatTime, transport: &Transport) -> f32 {
		let len = self.len();
		let end = self.end(transport);
		self.move_to(new_start);
		self.trim_end_to(end, transport);
		len / self.len()
	}

	pub fn stretch_end_to(&mut self, new_end: BeatTime, transport: &Transport) -> f32 {
		let len = self.len();
		self.trim_end_to(new_end, transport);
		len / self.len()
	}

	#[must_use]
	pub const fn position(self) -> BeatSpan {
		self.position
	}
}
