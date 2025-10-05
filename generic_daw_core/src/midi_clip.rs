use crate::{ClipPosition, Event, PatternId, daw_ctx::State};

#[derive(Clone, Copy, Debug)]
pub struct MidiClip {
	pub pattern: PatternId,
	pub position: ClipPosition,
}

impl MidiClip {
	pub fn collect_notes(&self, state: &State, notes: &mut [u8; 128]) {
		if state.rtstate.playing {
			let start_sample = state.rtstate.sample;

			let pattern = &state.patterns[*self.pattern];

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
	}

	pub fn process(
		&self,
		state: &State,
		audio: &[f32],
		events: &mut Vec<Event>,
		notes: &mut [u8; 128],
	) {
		let start_sample = state.rtstate.sample;
		let end_sample = start_sample + audio.len();

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
	}
}
