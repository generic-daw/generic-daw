use crate::{
	AudioClip, Event, MidiClip, MidiNote, Transport, audio_thread::State, midi_clip::VoiceId,
	time::BeatTime, voice_alloc::VoiceAlloc,
};
use utils::unique_id;

unique_id!(clip_id);

pub use clip_id::Id as ClipId;

#[derive(Clone, Copy, Debug)]
pub enum Clip {
	Audio(AudioClip),
	Midi(MidiClip),
}

impl Clip {
	#[must_use]
	pub fn id(&self) -> ClipId {
		match self {
			Self::Audio(clip) => clip.id,
			Self::Midi(clip) => clip.id,
		}
	}

	#[must_use]
	pub fn start(&self) -> BeatTime {
		match self {
			Self::Audio(clip) => clip.position.start(),
			Self::Midi(clip) => clip.position.start(),
		}
	}

	#[must_use]
	pub fn end(&self, transport: &Transport) -> BeatTime {
		match self {
			Self::Audio(clip) => clip.position.end(transport),
			Self::Midi(clip) => clip.position.end(),
		}
	}

	#[must_use]
	pub fn offset(&self, transport: &Transport) -> BeatTime {
		match self {
			Self::Audio(clip) => {
				(clip.position.offset() / clip.stretch.abs()).to_beat_time(transport)
			}
			Self::Midi(clip) => clip.position.offset(),
		}
	}

	#[must_use]
	pub fn len(&self, transport: &Transport) -> BeatTime {
		match self {
			Self::Audio(clip) => clip.position.len().to_beat_time(transport),
			Self::Midi(clip) => clip.position.len(),
		}
	}

	pub fn diff(
		&self,
		state: &State,
		audio: &[f32],
		events: &mut Vec<Event>,
		voice_alloc: &mut VoiceAlloc<VoiceId, MidiNote>,
	) {
		match self {
			Self::Audio(..) => {}
			Self::Midi(clip) => clip.diff(state, audio, events, voice_alloc),
		}
	}

	pub fn process(
		&self,
		state: &State,
		audio: &mut [f32],
		events: &mut Vec<Event>,
		voice_alloc: &mut VoiceAlloc<VoiceId, MidiNote>,
	) {
		match self {
			Self::Audio(clip) => clip.process(state, audio),
			Self::Midi(clip) => clip.process(state, audio, events, voice_alloc),
		}
	}

	pub fn trim_start_to(&mut self, new_start: BeatTime, transport: &Transport) {
		match self {
			Self::Audio(clip) => {
				clip.position
					.trim_start_to(new_start, transport, clip.stretch.abs());
			}
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

	pub fn slip_to(&mut self, new_offset: BeatTime, transport: &Transport) {
		match self {
			Self::Audio(clip) => clip
				.position
				.slip_to(new_offset * clip.stretch.abs(), transport),
			Self::Midi(clip) => clip.position.slip_to(new_offset),
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
