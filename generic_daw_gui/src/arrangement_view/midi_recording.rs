use crate::arrangement_view::midi_pattern::MidiPatternPair;
use generic_daw_core::{Event, MidiKey, MidiNote, NoteId, MusicalTime, Position, Transport};
use std::collections::HashMap;

#[derive(Debug)]
pub struct MidiRecording {
	start_sample: usize,
	active: HashMap<NoteId, (u8, f32, usize)>,
	notes: Vec<MidiNote>,
}

impl MidiRecording {
	pub fn new(transport: &Transport) -> Self {
		Self {
			start_sample: transport.sample,
			active: HashMap::new(),
			notes: Vec::new(),
		}
	}

	pub fn start_position(&self, transport: &Transport) -> MusicalTime {
		MusicalTime::from_samples(self.start_sample, transport)
	}

	pub fn on_event(&mut self, event: Event, transport: &Transport) {
		let time = match event {
			Event::On { time, .. }
			| Event::Off { time, .. }
			| Event::Choke { time, .. }
			| Event::End { time, .. }
			| Event::ParamValue { time, .. } => time as usize,
		};
		let sample = transport.sample + time * 2;

		match event {
			Event::On {
				key,
				velocity,
				note_id,
				..
			} => {
				self.active.insert(
					note_id,
					(
						key,
						velocity,
						sample.saturating_sub(self.start_sample),
					),
				);
			}
			Event::Off { note_id, .. } | Event::Choke { note_id: Some(note_id), .. } | Event::End {
				note_id: Some(note_id),
				..
			} => {
				self.finish_note(note_id, sample.saturating_sub(self.start_sample), transport);
			}
			Event::Choke { key, note_id: None, .. } | Event::End { key, note_id: None, .. } => {
				let note_ids = self
					.active
					.iter()
					.filter_map(|(&note_id, &(active_key, ..))| (active_key == key).then_some(note_id))
					.collect::<Vec<_>>();
				for note_id in note_ids {
					self.finish_note(note_id, sample.saturating_sub(self.start_sample), transport);
				}
			}
			Event::ParamValue { .. } => {}
		}
	}

	pub fn finalize(mut self, transport: &Transport) -> Option<MidiPatternPair> {
		let end = transport.sample.saturating_sub(self.start_sample);
		let pending = self.active.keys().copied().collect::<Vec<_>>();
		for note_id in pending {
			self.finish_note(note_id, end, transport);
		}

		self.notes.sort_by_key(|note| {
			(
				note.position.start().into_raw(),
				note.position.end().into_raw(),
				note.key.0,
			)
		});

		(!self.notes.is_empty()).then(|| MidiPatternPair::from_notes(self.notes, "MIDI Recording"))
	}

	fn finish_note(&mut self, note_id: NoteId, end: usize, transport: &Transport) {
		let Some((key, velocity, start)) = self.active.remove(&note_id) else {
			return;
		};

		self.notes.push(MidiNote::new(
			MidiKey(key),
			velocity,
			Position::new(
				MusicalTime::from_samples(start, transport),
				MusicalTime::from_samples(end, transport),
			),
		));
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::num::NonZero;

	#[test]
	fn finalize_closes_held_notes_at_stop() {
		let mut transport = Transport::new(NonZero::new(48_000).unwrap(), NonZero::new(2).unwrap());
		let mut recording = MidiRecording::new(&transport);
		let note_id = NoteId::unique();

		recording.on_event(
			Event::On {
				time: 0,
				key: 60,
				velocity: 1.0,
				note_id,
			},
			&transport,
		);

		transport.sample = 8;

		let pattern = recording.finalize(&transport).unwrap();
		let [note] = pattern.gui.notes.as_slice() else {
			panic!("expected one note");
		};

		assert_eq!(note.key.0, 60);
		assert_eq!(note.position.start(), MusicalTime::ZERO);
		assert_eq!(note.position.end(), MusicalTime::from_samples(8, &transport));
	}
}
