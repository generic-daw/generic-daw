use std::sync::Arc;

#[derive(Debug)]
pub struct LiveSample {
	audio: Arc<[f32]>,
	idx: isize,
}

impl LiveSample {
	#[must_use]
	pub fn new(audio: Arc<[f32]>, offset: usize) -> Self {
		Self {
			audio,
			idx: -(offset as isize),
		}
	}

	pub fn process(&mut self, audio: &mut [f32]) {
		let idx = self.idx;
		let uidx = idx.unsigned_abs();
		self.idx += audio.len() as isize;

		if idx > 0 {
			self.audio[uidx..]
				.iter()
				.zip(audio)
				.for_each(|(sample, buf)| *buf += sample);
		} else {
			if uidx >= audio.len() {
				return;
			}

			self.audio
				.iter()
				.zip(&mut audio[uidx..])
				.for_each(|(sample, buf)| *buf += sample);
		}
	}

	pub fn over(&self) -> bool {
		self.idx >= self.audio.len() as isize
	}
}
