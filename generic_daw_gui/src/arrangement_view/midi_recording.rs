use crate::arrangement_view::midi_pattern::MidiPatternPair;
use generic_daw_core::{
	MidiAction, MidiKey, MidiNote, MidiNoteId, Transport,
	time::{BeatRange, BeatTime},
	u4, u7,
};
use std::{collections::HashMap, sync::Arc};

#[derive(Debug)]
pub struct MidiRecording {
	pub notes: Vec<MidiNote>,
	pub playing: HashMap<(u4, u7), (u7, BeatTime)>,
	pub position: BeatTime,
	pub name: Arc<str>,
	pub dropping: bool,
}

impl MidiRecording {
	pub fn new(name: Arc<str>, transport: &Transport) -> Self {
		Self {
			notes: Vec::new(),
			playing: HashMap::new(),
			position: transport.position.to_beat_time(transport),
			name,
			dropping: false,
		}
	}

	pub fn recorded(&mut self, actions: &[MidiAction], transport: &Transport) {
		let pos_in_clip = transport.position.to_beat_time(transport) - self.position;

		for &action in actions {
			if let Some((velocity, position)) = match action {
				MidiAction::NoteOn(channel, key, velocity) => {
					self.playing.insert((channel, key), (velocity, pos_in_clip))
				}
				MidiAction::NoteOff(channel, key, _) => self.playing.remove(&(channel, key)),
			} && position != pos_in_clip
			{
				self.notes.push(MidiNote {
					id: MidiNoteId::unique(),
					key: MidiKey(action.key().as_int()),
					velocity: f32::from(velocity.as_int()) / 127.0,
					position: BeatRange::new(position, pos_in_clip),
				});
			}
		}
	}

	pub fn finalize(mut self, transport: &Transport) -> (BeatTime, MidiPatternPair) {
		let transport_beats = transport.position.to_beat_time(transport);

		for ((_, key), (velocity, position)) in self.playing {
			self.notes.push(MidiNote {
				id: MidiNoteId::unique(),
				key: MidiKey(key.as_int()),
				velocity: f32::from(velocity.as_int()) / 127.0,
				position: BeatRange::new(position, transport_beats),
			});
		}

		(
			self.position,
			MidiPatternPair::from_notes(self.notes, &self.name),
		)
	}
}
