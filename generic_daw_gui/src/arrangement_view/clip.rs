use crate::arrangement_view::{audio_clip::AudioClip, midi_clip::MidiClip};
use generic_daw_core::{self as core, ClipPosition, MusicalTime};
use generic_daw_utils::NoDebug;

#[derive(Clone, Copy, Debug)]
pub enum Clip {
	Audio(AudioClip),
	Midi(MidiClip),
}

impl Clip {
	pub fn position(&self) -> &ClipPosition {
		match self {
			Self::Audio(audio) => &audio.position,
			Self::Midi(midi) => &midi.position,
		}
	}

	pub fn move_to(&mut self, pos: MusicalTime) {
		match self {
			Self::Audio(audio) => audio.position.move_to(pos),
			Self::Midi(midi) => midi.position.move_to(pos),
		}
	}

	pub fn trim_start_to(&mut self, pos: MusicalTime) {
		match self {
			Self::Audio(audio) => audio.position.trim_start_to(pos),
			Self::Midi(midi) => midi.position.trim_start_to(pos),
		}
	}

	pub fn trim_end_to(&mut self, pos: MusicalTime) {
		match self {
			Self::Audio(audio) => audio.position.trim_end_to(pos),
			Self::Midi(midi) => midi.position.trim_end_to(pos),
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

impl From<Clip> for core::Clip {
	fn from(value: Clip) -> Self {
		match value {
			Clip::Audio(AudioClip { sample, position }) => {
				Self::Audio(core::AudioClip { sample, position })
			}
			Clip::Midi(MidiClip { pattern, position }) => Self::Midi(core::MidiClip {
				pattern,
				position,
				notes: NoDebug(Box::new([0; 128])),
			}),
		}
	}
}
