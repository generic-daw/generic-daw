use crate::{MusicalTime, NotePosition};

#[derive(Clone, Copy, Debug, Default)]
pub struct ClipPosition {
	position: NotePosition,
	offset: MusicalTime,
}

impl ClipPosition {
	#[must_use]
	pub fn new(position: NotePosition, offset: MusicalTime) -> Self {
		debug_assert!(offset < position.len());
		Self { position, offset }
	}

	#[must_use]
	pub fn start(self) -> MusicalTime {
		self.position.start()
	}

	#[must_use]
	pub fn end(self) -> MusicalTime {
		self.position.end()
	}

	#[must_use]
	pub fn offset(self) -> MusicalTime {
		self.offset
	}

	#[must_use]
	pub fn len(self) -> MusicalTime {
		self.end() - self.start()
	}

	pub fn trim_start_to(&mut self, mut new_start: MusicalTime) {
		let old_start = self.start();
		if self.offset() + new_start < old_start {
			new_start = old_start - self.offset();
		}
		self.position.trim_start_to(new_start);
		new_start = self.start();
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
}
