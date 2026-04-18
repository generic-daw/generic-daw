use crate::{MusicalTime, Position, Transport};

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct OffsetPosition {
	position: Position,
	offset: MusicalTime,
}

impl OffsetPosition {
	#[must_use]
	pub fn new(position: Position, offset: MusicalTime) -> Self {
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
	pub const fn start(self) -> MusicalTime {
		self.position.start()
	}

	#[must_use]
	pub const fn end(self) -> MusicalTime {
		self.position.end()
	}

	#[must_use]
	pub const fn offset(self) -> MusicalTime {
		self.offset
	}

	#[must_use]
	pub fn len(self) -> MusicalTime {
		self.position.len()
	}

	pub fn trim_start_to(&mut self, new_start: MusicalTime) {
		let old_start = self.start();
		self.position
			.trim_start_to(new_start.max(old_start.saturating_sub(self.offset())));
		let new_start = self.start();
		let diff = new_start.abs_diff(old_start);
		if old_start < new_start {
			self.offset += diff;
		} else {
			self.offset -= diff;
		}
	}

	pub fn trim_end_to(&mut self, new_end: MusicalTime) {
		self.position.trim_end_to(new_end);
	}

	pub fn move_to(&mut self, new_start: MusicalTime) {
		self.position.move_to(new_start);
	}

	pub fn stretch_start_to(&mut self, new_start: MusicalTime) -> f32 {
		let old_len = self.len();
		self.position.trim_start_to(new_start);
		let fac = old_len / self.len();
		self.offset /= fac;
		fac
	}

	pub fn stretch_end_to(&mut self, new_end: MusicalTime) -> f32 {
		let old_len = self.len();
		self.trim_end_to(new_end);
		let fac = old_len / self.len();
		self.offset /= fac;
		fac
	}

	#[must_use]
	pub fn position(self) -> Position {
		self.position
	}

	#[must_use]
	pub fn stretch(self, stretch: f32) -> Self {
		Self::new(self.position().stretch(stretch), self.offset() * stretch)
	}
}

impl From<Position> for OffsetPosition {
	fn from(value: Position) -> Self {
		Self::new(value, MusicalTime::ZERO)
	}
}
