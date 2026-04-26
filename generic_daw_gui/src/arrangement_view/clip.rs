use crate::arrangement_view::{audio_clip::AudioClip, midi_clip::MidiClip};
use generic_daw_core::{Transport, time::BeatTime};

#[derive(Clone, Copy, Debug)]
pub enum Clip {
	Audio(AudioClip),
	Midi(MidiClip),
}

impl Clip {
	pub fn start(&self) -> BeatTime {
		match self {
			Self::Audio(clip) => clip.position.start(),
			Self::Midi(clip) => clip.position.start(),
		}
	}

	pub fn end(&self, transport: &Transport) -> BeatTime {
		match self {
			Self::Audio(clip) => clip.position.end(transport),
			Self::Midi(clip) => clip.position.end(),
		}
	}

	pub fn offset(&self, transport: &Transport) -> BeatTime {
		match self {
			Self::Audio(clip) => (clip.position.offset() / clip.stretch).to_beat_time(transport),
			Self::Midi(clip) => clip.position.offset(),
		}
	}

	pub fn len(&self, transport: &Transport) -> BeatTime {
		match self {
			Self::Audio(clip) => clip.position.len().to_beat_time(transport),
			Self::Midi(clip) => clip.position.len(),
		}
	}

	pub fn trim_start_to(&mut self, new_start: BeatTime, transport: &Transport) {
		match self {
			Self::Audio(clip) => clip
				.position
				.trim_start_to(new_start, transport, clip.stretch),
			Self::Midi(clip) => clip.position.trim_start_to(new_start),
		}
	}

	pub fn trim_end_to(&mut self, new_end: BeatTime, transport: &Transport) {
		match self {
			Self::Audio(clip) => clip.position.trim_end_to(new_end, transport),
			Self::Midi(clip) => clip.position.trim_end_to(new_end),
		}
	}

	pub fn move_to(&mut self, new_start: BeatTime) {
		match self {
			Self::Audio(clip) => clip.position.move_to(new_start),
			Self::Midi(clip) => clip.position.move_to(new_start),
		}
	}

	pub fn slip_to(&mut self, new_start: BeatTime, transport: &Transport) {
		match self {
			Self::Audio(clip) => clip.position.slip_to(new_start * clip.stretch, transport),
			Self::Midi(clip) => clip.position.slip_to(new_start),
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
