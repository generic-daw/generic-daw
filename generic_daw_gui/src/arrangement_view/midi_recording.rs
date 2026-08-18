use crate::arrangement_view::midi_pattern::MidiPatternPair;
use generic_daw_core::{
	MidiAction, MidiKey, MidiNote, MidiNoteId, TimedMidiAction, Transport,
	time::{BeatRange, BeatTime},
	u4, u7,
};
use std::{collections::HashMap, sync::Arc};

#[derive(Debug)]
pub struct MidiRecording {
	pub notes: Vec<MidiNote>,
	pub playing: HashMap<(u4, u7), (u7, BeatTime)>,
	pub position: BeatTime,
	pub frames: usize,
	pub name: Arc<str>,
	pub dropping: bool,
}

impl MidiRecording {
	pub fn new(name: Arc<str>, transport: &Transport) -> Self {
		Self {
			notes: Vec::new(),
			playing: HashMap::new(),
			position: transport.position.to_beat_time(transport),
			frames: 0,
			name,
			dropping: false,
		}
	}

	pub fn end(&self, transport: &Transport) -> BeatTime {
		self.position + self.len(transport)
	}

	pub fn len(&self, transport: &Transport) -> BeatTime {
		BeatTime::from_frames(self.frames, transport)
	}

	pub fn recorded(&mut self, actions: &[TimedMidiAction<BeatTime>], frames: usize) {
		self.frames += frames;

		for &action in actions {
			if let Some((velocity, position)) = match action.action {
				MidiAction::NoteOn(channel, key, velocity) => self
					.playing
					.insert((channel, key), (velocity, action.ts - self.position)),
				MidiAction::NoteOff(channel, key, _) => self.playing.remove(&(channel, key)),
			} {
				self.notes.push(MidiNote {
					id: MidiNoteId::unique(),
					key: MidiKey(action.action.key().as_int()),
					velocity: f32::from(velocity.as_int()) / 127.0,
					position: BeatRange::new(position, action.ts - self.position),
				});
			}
		}
	}

	pub fn finalize(mut self, transport: &Transport) -> (BeatRange, MidiPatternPair) {
		for ((_, key), &(velocity, position)) in &self.playing {
			self.notes.push(MidiNote {
				id: MidiNoteId::unique(),
				key: MidiKey(key.as_int()),
				velocity: f32::from(velocity.as_int()) / 127.0,
				position: BeatRange::new(position, self.len(transport)),
			});
		}

		(
			BeatRange::new(self.position, self.end(transport)),
			MidiPatternPair::from_notes(self.notes, &self.name),
		)
	}
}
