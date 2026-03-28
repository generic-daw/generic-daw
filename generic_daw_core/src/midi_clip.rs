use crate::{Event, MidiPatternId, OffsetPosition, audio_processor::State, track::ActiveNote};

#[derive(Clone, Copy, Debug)]
pub struct MidiClip {
	pub pattern: MidiPatternId,
	pub position: OffsetPosition,
}

impl MidiClip {
	pub(crate) fn collect_notes(&self, state: &State, notes: &mut Vec<ActiveNote>) {
		debug_assert!(state.transport.playing);

		state.midi_patterns[&self.pattern]
			.notes
			.iter()
			.filter_map(|&note| {
				let mut note = note;
				note.position = (note.position + self.position.start())
					.saturating_sub(self.position.offset())?
					.clamp(self.position.position())?;
				Some(note)
			})
			.for_each(|note| {
				let (start, end) = note.position.to_samples(&state.transport);
				if start < state.transport.sample && end >= state.transport.sample {
					notes.push(ActiveNote {
						note_id: note.id,
						key: note.key.0,
						velocity: note.velocity,
					});
				}
			});
	}

	pub fn process(&self, state: &State, audio: &[f32], events: &mut Vec<Event>) {
		debug_assert!(state.transport.playing);

		state.midi_patterns[&self.pattern]
			.notes
			.iter()
			.filter_map(|&note| {
				let mut note = note;
				note.position = (note.position + self.position.start())
					.saturating_sub(self.position.offset())?
					.clamp(self.position.position())?;
				Some(note)
			})
			.for_each(|note| {
				let (start, end) = note.position.to_samples(&state.transport);

				if let Some(time) = start.checked_sub(state.transport.sample)
					&& time < audio.len()
				{
					events.push(Event::On {
						time: time as u32 / 2,
						key: note.key.0,
						velocity: note.velocity,
						note_id: note.id,
					});
				}

				if let Some(time) = end.checked_sub(state.transport.sample)
					&& time < audio.len()
				{
					events.push(Event::Off {
						time: time as u32 / 2,
						key: note.key.0,
						velocity: note.velocity,
						note_id: note.id,
					});
				}
			});
	}
}
