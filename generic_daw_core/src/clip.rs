use crate::{AudioClip, Event, MidiClip, OffsetPosition, daw_ctx::State};

#[derive(Clone, Copy, Debug)]
pub enum Clip {
	Audio(AudioClip),
	Midi(MidiClip),
}

impl Clip {
	pub fn process(
		&self,
		state: &State,
		audio: &mut [f32],
		events: &mut Vec<Event>,
		notes: &mut [u8; 128],
	) {
		match self {
			Self::Audio(clip) => clip.process(state, audio),
			Self::Midi(clip) => clip.process(state, audio, events, notes),
		}
	}

	pub fn position(&mut self) -> &mut OffsetPosition {
		match self {
			Self::Audio(clip) => &mut clip.position,
			Self::Midi(clip) => &mut clip.position,
		}
	}
}

impl From<AudioClip> for Clip {
	fn from(value: AudioClip) -> Self {
		Self::Audio(value)
	}
}

impl From<MidiClip> for Clip {
	fn from(value: MidiClip) -> Self {
		Self::Midi(value)
	}
}
