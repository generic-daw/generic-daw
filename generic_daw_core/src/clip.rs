use crate::{
	AudioClip, Event, MidiClip, MidiNote, Transport, audio_thread::State, midi_clip::VoiceId,
	time::BeatTime, voice_alloc::VoiceAlloc,
};
use utils::unique_id;

unique_id!(ClipId);

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
			Self::Midi(clip) => clip.position.offset() / clip.stretch.abs(),
		}
	}

	#[must_use]
	pub fn len(&self, transport: &Transport) -> BeatTime {
		match self {
			Self::Audio(clip) => clip.position.len().to_beat_time(transport),
			Self::Midi(clip) => clip.position.len(),
		}
	}

	#[must_use]
	pub fn stretch(&self) -> f64 {
		match self {
			Self::Audio(clip) => clip.stretch,
			Self::Midi(clip) => clip.stretch,
		}
	}

	pub fn diff(
		&self,
		state: &State,
		events: &mut Vec<Event>,
		voice_alloc: &mut VoiceAlloc<VoiceId, MidiNote>,
	) {
		match self {
			Self::Audio(..) => {}
			Self::Midi(clip) => clip.diff(state, events, voice_alloc),
		}
	}

	pub fn process(
		&self,
		state: &State,
		audio: &mut [[f32; 2]],
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
				clip.fade_start.len = clip.fade_start.len.min(clip.position.len());
				clip.fade_end.len = clip
					.fade_end
					.len
					.min(clip.position.len() - clip.fade_start.len);
			}
			Self::Midi(clip) => clip.position.trim_start_to(new_start, clip.stretch.abs()),
		}
	}

	pub fn trim_end_to(&mut self, new_end: BeatTime, transport: &Transport) {
		match self {
			Self::Audio(clip) => {
				clip.position.trim_end_to(new_end, transport);
				clip.fade_end.len = clip.fade_end.len.min(clip.position.len());
				clip.fade_start.len = clip
					.fade_start
					.len
					.min(clip.position.len() - clip.fade_end.len);
			}
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
		let len = self.len(transport) * self.stretch().abs();
		let end = self.end(transport);

		self.move_to(new_start.min(end - BeatTime::TICK));
		self.trim_end_to(end, transport);

		let new_stretch = (len / self.len(transport))
			.clamp(0.001, 1000.0)
			.copysign(self.stretch());

		let new_start = end - len / new_stretch.abs();

		self.move_to(new_start);
		self.trim_end_to(end, transport);

		match self {
			Self::Audio(clip) => {
				let old_stretch = clip.stretch;
				clip.stretch = new_stretch;
				clip.fade_start.len = clip.fade_start.len * old_stretch / clip.stretch;
				clip.fade_end.len = clip.fade_end.len * old_stretch / clip.stretch;
			}
			Self::Midi(clip) => clip.stretch = new_stretch,
		}
	}

	pub fn stretch_end_to(&mut self, new_end: BeatTime, transport: &Transport) {
		let len = self.len(transport) * self.stretch().abs();

		self.trim_end_to(new_end, transport);

		let new_stretch = (len / self.len(transport))
			.clamp(0.001, 1000.0)
			.copysign(self.stretch());

		let new_end = self.start() + len / new_stretch.abs();

		self.trim_end_to(new_end, transport);

		match self {
			Self::Audio(clip) => {
				let old_stretch = clip.stretch;
				clip.stretch = new_stretch;
				clip.fade_start.len = clip.fade_start.len * old_stretch / clip.stretch;
				clip.fade_end.len = clip.fade_end.len * old_stretch / clip.stretch;
			}
			Self::Midi(clip) => clip.stretch = new_stretch,
		}
	}

	pub fn slip_to(&mut self, new_offset: BeatTime, transport: &Transport) {
		match self {
			Self::Audio(clip) => clip
				.position
				.slip_to(new_offset * clip.stretch.abs(), transport),
			Self::Midi(clip) => clip.position.slip_to(new_offset * clip.stretch.abs()),
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
