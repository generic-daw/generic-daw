use crate::{ClipPosition, Event, PatternId, daw_ctx::State};

#[derive(Clone, Copy, Debug)]
pub struct MidiClip {
	pub pattern: PatternId,
	pub position: ClipPosition,
}

impl MidiClip {
	pub fn collect_notes(&self, state: &State, notes: &mut [u8; 128]) {
		debug_assert!(state.rtstate.playing);

		state.patterns[*self.pattern]
			.notes
			.iter()
			.filter_map(|&(mut note)| {
				note.position = (note.position + self.position.start())
					.saturating_sub(self.position.offset())?
					.clamp(self.position.note_position())?;
				Some(note)
			})
			.for_each(|note| {
				let start = note.position.start().to_samples(&state.rtstate);
				let end = note.position.end().to_samples(&state.rtstate);

				if start < state.rtstate.sample && end >= state.rtstate.sample {
					notes[usize::from(note.key.0)] += 1;
				}
			});
	}

	pub fn process(
		&self,
		state: &State,
		audio: &[f32],
		events: &mut Vec<Event>,
		notes: &mut [u8; 128],
	) {
		debug_assert!(state.rtstate.playing);

		state.patterns[*self.pattern]
			.notes
			.iter()
			.filter_map(|&(mut note)| {
				note.position = (note.position + self.position.start())
					.saturating_sub(self.position.offset())?
					.clamp(self.position.note_position())?;
				Some(note)
			})
			.for_each(|note| {
				let start = note.position.start().to_samples(&state.rtstate);
				let end = note.position.end().to_samples(&state.rtstate);

				if let Some(time) = start.checked_sub(state.rtstate.sample)
					&& time < audio.len()
				{
					events.push(Event::On {
						time: time as u32 / 2,
						key: note.key.0,
						velocity: note.velocity,
					});
					notes[usize::from(note.key.0)] += 1;
				}

				if let Some(time) = end.checked_sub(state.rtstate.sample)
					&& time < audio.len()
				{
					events.push(Event::End {
						time: time as u32 / 2,
						key: note.key.0,
					});
					notes[usize::from(note.key.0)] -= 1;
				}
			});
	}
}
