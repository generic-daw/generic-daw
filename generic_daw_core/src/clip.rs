use crate::{
	AudioClip, Event, MidiClip, Transport, VoiceAlloc, audio_processor::State, time::BeatTime,
};

#[derive(Clone, Copy, Debug)]
pub enum Clip {
	Audio(AudioClip),
	Midi(MidiClip),
}

impl Clip {
	pub fn diff(
		&self,
		state: &State,
		audio: &[f32],
		events: &mut Vec<Event>,
		voice_alloc: &mut VoiceAlloc,
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
		voice_alloc: &mut VoiceAlloc,
	) {
		match self {
			Self::Audio(clip) => clip.process(state, audio),
			Self::Midi(clip) => clip.process(state, audio, events, voice_alloc),
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

	pub fn stretch_start_to(&mut self, new_start: BeatTime, transport: &Transport) {
		match self {
			Self::Audio(clip) => {
				clip.stretch *= clip.position.stretch_start_to(new_start, transport);
				clip.stretch = clip.stretch.clamp(2f32.powi(-10), 2f32.powi(10));
			}
			Self::Midi(..) => panic!(),
		}
	}

	pub fn stretch_end_to(&mut self, new_end: BeatTime, transport: &Transport) {
		match self {
			Self::Audio(clip) => {
				clip.stretch *= clip.position.stretch_end_to(new_end, transport);
				clip.stretch = clip.stretch.clamp(2f32.powi(-10), 2f32.powi(10));
			}
			Self::Midi(..) => panic!(),
		}
	}

	pub fn slip_to(&mut self, new_offset: BeatTime, transport: &Transport) {
		match self {
			Self::Audio(clip) => clip.position.slip_to(new_offset * clip.stretch, transport),
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
