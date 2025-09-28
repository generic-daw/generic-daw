use crate::MusicalTime;

#[derive(Clone, Copy, Debug, Default)]
pub struct NotePosition {
	start: MusicalTime,
	end: MusicalTime,
}

impl NotePosition {
	#[must_use]
	pub fn new(start: MusicalTime, end: MusicalTime) -> Self {
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
		self.start = new_start.clamp(self.start, self.end - MusicalTime::TICK);
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
