use std::{cmp::Ordering, fmt::Debug, num::NonZero};

#[derive(Clone, Copy, Debug)]
pub struct Voice<Id, Info> {
	pub id: Id,
	pub note_id: u32,
	pub active: bool,
	pub info: Info,
}

#[derive(Debug)]
pub struct VoiceAlloc<Id, Info> {
	voices: Vec<Voice<Id, Info>>,
	next_note_id: u32,
}

impl<Id: Copy + Eq, Info: Copy> VoiceAlloc<Id, Info> {
	pub fn new(count: NonZero<usize>) -> Self {
		Self {
			voices: Vec::with_capacity(count.get()),
			next_note_id: 0,
		}
	}

	pub fn current_polyphony(&self) -> usize {
		self.voices.len()
	}

	pub fn deactivate_all(&mut self) {
		for voice in &mut self.voices {
			voice.active = false;
		}
	}

	pub fn drain_inactive(&mut self) -> impl Iterator<Item = Voice<Id, Info>> {
		self.voices.extract_if(.., |voice| !voice.active)
	}

	pub fn activate(&mut self, id: Id) -> bool {
		if let Some(voice) = self.voices.iter_mut().find(|voice| voice.id == id) {
			voice.active = true;
			true
		} else {
			false
		}
	}

	pub fn alloc(&mut self, id: Id, info: Info) -> Option<Voice<Id, Info>> {
		if self.voices.len() == self.voices.capacity() {
			return None;
		}

		let voice = self.new_voice(id, info);

		self.voices.push(voice);

		Some(voice)
	}

	pub fn steal(
		&mut self,
		id: Id,
		info: Info,
		mut f: impl FnMut(&Voice<Id, Info>, &Voice<Id, Info>) -> Ordering,
	) -> (Voice<Id, Info>, Voice<Id, Info>) {
		let voice = self.new_voice(id, info);

		let old_voice = std::mem::replace(
			self.voices.iter_mut().min_by(|l, r| f(l, r)).unwrap(),
			voice,
		);

		(voice, old_voice)
	}

	fn new_voice(&mut self, id: Id, info: Info) -> Voice<Id, Info> {
		let note_id = self.next_note_id;
		self.next_note_id = (self.next_note_id + 1) % i32::MAX as u32;

		Voice {
			id,
			note_id,
			active: true,
			info,
		}
	}

	pub fn dealloc(&mut self, id: Id) -> Option<Voice<Id, Info>> {
		self.voices
			.iter()
			.position(|voice| voice.id == id)
			.map(|i| self.voices.swap_remove(i))
	}
}
