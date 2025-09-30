use crate::{AudioClip, ClipPosition, Event, MidiClip, daw_ctx::State};

#[derive(Clone, Debug)]
pub enum Clip {
	Audio(AudioClip),
	Midi(MidiClip),
}

impl Clip {
	pub fn process(&mut self, state: &State, audio: &mut [f32], events: &mut Vec<Event>) {
		match self {
			Self::Audio(clip) => clip.process(state, audio),
			Self::Midi(clip) => clip.process(state, audio, events),
		}
	}

	pub fn position(&mut self) -> &mut ClipPosition {
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
