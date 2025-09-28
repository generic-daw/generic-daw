use crate::{ClipPosition, Event, PatternId, daw_ctx::State};
use generic_daw_utils::NoDebug;
use std::{cmp::Ordering, iter::repeat_n};

#[derive(Debug)]
pub struct MidiClip {
	pub pattern: PatternId,
	pub position: ClipPosition,
	pub notes: NoDebug<Box<[u8; 128]>>,
}

impl Clone for MidiClip {
	fn clone(&self) -> Self {
		Self {
			pattern: self.pattern,
			position: self.position,
			notes: Box::new([0; 128]).into(),
		}
	}
}

impl MidiClip {
	#[must_use]
	pub fn new(pattern: PatternId) -> Self {
		Self {
			pattern,
			position: ClipPosition::default(),
			notes: Box::new([0; 128]).into(),
		}
	}

	pub fn process(&mut self, state: &State, audio: &[f32], events: &mut Vec<Event>) {
		let start_sample = state.rtstate.sample;
		let end_sample = start_sample + audio.len();

		let mut notes = [0; 128];

		let pattern = &state.patterns[*self.pattern];

		if state.rtstate.playing {
			pattern
				.notes
				.iter()
				.map(|&note| note + self.position.start())
				.filter(|note| note.position.start() >= self.position.offset())
				.map(|note| note - self.position.offset())
				.for_each(|note| {
					let start = note.position.start().to_samples(&state.rtstate);
					let end = note.position.end().to_samples(&state.rtstate);

					if start < start_sample && end >= start_sample {
						notes[note.key.0 as usize] += 1;
					}
				});
		}

		self.notes
			.iter()
			.zip(notes)
			.enumerate()
			.for_each(|(key, (before, after))| {
				let event = match before.cmp(&after) {
					Ordering::Equal => return,
					Ordering::Less => Event::On {
						time: 0,
						key: key as u8,
						velocity: 1.0,
					},
					Ordering::Greater => Event::Off {
						time: 0,
						key: key as u8,
						velocity: 1.0,
					},
				};

				events.extend(repeat_n(event, before.abs_diff(after) as usize));
			});

		if state.rtstate.playing {
			pattern
				.notes
				.iter()
				.map(|&note| note + self.position.start())
				.filter(|note| note.position.start() >= self.position.offset())
				.map(|note| note - self.position.offset())
				.for_each(|note| {
					let start = note.position.start().to_samples(&state.rtstate);
					let end = note.position.end().to_samples(&state.rtstate);

					if start >= start_sample && start < end_sample {
						events.push(Event::On {
							time: (start - start_sample) as u32 / 2,
							key: note.key.0,
							velocity: note.velocity,
						});
						notes[note.key.0 as usize] += 1;
					}

					if end >= start_sample && end < end_sample {
						events.push(Event::End {
							time: (end - start_sample) as u32 / 2,
							key: note.key.0,
						});
						notes[note.key.0 as usize] -= 1;
					}
				});
		}

		**self.notes = notes;
	}
}
