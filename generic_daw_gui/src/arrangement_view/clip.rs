use crate::arrangement_view::{audio_clip::AudioClip, midi_clip::MidiClip};
use generic_daw_core::OffsetPosition;

#[derive(Clone, Copy, Debug)]
pub enum Clip {
	Audio(AudioClip),
	Midi(MidiClip),
}

impl Clip {
	pub fn position(&self) -> &OffsetPosition {
		match self {
			Self::Audio(audio) => &audio.position,
			Self::Midi(midi) => &midi.position,
		}
	}

	pub fn position_mut(&mut self) -> &mut OffsetPosition {
		match self {
			Self::Audio(audio) => &mut audio.position,
			Self::Midi(midi) => &mut midi.position,
		}
	}

	pub fn stretch(&mut self) -> &mut f32 {
		match self {
			Self::Audio(audio) => &mut audio.stretch,
			Self::Midi(..) => panic!(),
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

impl From<Clip> for generic_daw_core::Clip {
	fn from(value: Clip) -> Self {
		match value {
			Clip::Audio(AudioClip {
				sample,
				position,
				stretch,
			}) => Self::Audio(generic_daw_core::AudioClip {
				sample,
				position,
				stretch,
			}),
			Clip::Midi(MidiClip {
				id,
				pattern,
				position,
			}) => Self::Midi(generic_daw_core::MidiClip {
				id,
				pattern,
				position,
			}),
		}
	}
}
