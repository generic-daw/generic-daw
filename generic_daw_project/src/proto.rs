use prost::{Message, Oneof};
use std::ffi::CStr;

#[derive(Clone, Copy, Eq, Hash, Message, PartialEq)]
pub struct SampleIndex {
	#[prost(uint32)]
	pub(crate) index: u32,
}

#[derive(Clone, Copy, Eq, Hash, Message, PartialEq)]
pub struct PatternIndex {
	#[prost(uint32)]
	pub(crate) index: u32,
}

#[derive(Clone, Copy, Eq, Hash, Message, PartialEq)]
pub struct TrackIndex {
	#[prost(uint32)]
	pub(crate) index: u32,
}

#[derive(Clone, Copy, Eq, Hash, Message, PartialEq)]
pub struct ChannelIndex {
	#[prost(uint32)]
	pub(crate) index: u32,
}

#[derive(Message)]
pub struct Project {
	#[prost(message, required)]
	pub rtstate: RtState,
	#[prost(message, repeated)]
	pub samples: Vec<Sample>,
	#[prost(message, repeated)]
	pub patterns: Vec<Pattern>,
	#[prost(message, repeated)]
	pub tracks: Vec<Track>,
	#[prost(message, repeated)]
	pub channels: Vec<Channel>,
}

#[derive(Clone, Copy, Message)]
pub struct RtState {
	#[prost(uint32)]
	pub bpm: u32,
	#[prost(uint32)]
	pub numerator: u32,
}

#[derive(Message)]
pub struct Sample {
	#[prost(string)]
	pub name: String,
	#[prost(uint32)]
	pub crc: u32,
}

#[derive(Message)]
pub struct Pattern {
	#[prost(message, repeated)]
	pub notes: Vec<Note>,
}

#[derive(Message)]
pub struct Track {
	#[prost(message, repeated)]
	pub clips: Vec<OptionClip>,
	#[prost(message, required)]
	pub channel: Channel,
}

#[derive(Message)]
pub struct Channel {
	#[prost(message, repeated)]
	pub connections: Vec<ChannelIndex>,
	#[prost(message, repeated)]
	pub plugins: Vec<Plugin>,
	#[prost(float, default = 1.0)]
	pub volume: f32,
	#[prost(message, required)]
	pub pan: OptionPanMode,
}

#[derive(Clone, Copy, Message)]
pub struct OptionPanMode {
	#[prost(oneof = "PanMode", tags = "1, 2")]
	pub pan_mode: Option<PanMode>,
}

#[derive(Clone, Copy, Oneof, PartialEq)]
pub enum PanMode {
	#[prost(message, tag = "1")]
	Balance(PanModeBalance),
	#[prost(message, tag = "2")]
	Stereo(PanModeStereo),
}

#[derive(Clone, Copy, Message, PartialEq)]
pub struct PanModeBalance {
	#[prost(float, default = 0.0)]
	pub pan: f32,
}

#[derive(Clone, Copy, Message, PartialEq)]
pub struct PanModeStereo {
	#[prost(float, default = -1.0)]
	pub l: f32,
	#[prost(float, default = 1.0)]
	pub r: f32,
}

impl From<PanModeBalance> for OptionPanMode {
	fn from(value: PanModeBalance) -> Self {
		Self {
			pan_mode: Some(PanMode::Balance(value)),
		}
	}
}

impl From<PanModeStereo> for OptionPanMode {
	fn from(value: PanModeStereo) -> Self {
		Self {
			pan_mode: Some(PanMode::Stereo(value)),
		}
	}
}

#[derive(Clone, Copy, Message)]
pub struct Note {
	#[prost(uint32)]
	pub key: u32,
	#[prost(float, default = 1.0)]
	pub velocity: f32,
	#[prost(message, required)]
	pub position: NotePosition,
}

#[derive(Clone, Copy, Message)]
pub struct OptionClip {
	#[prost(oneof = "Clip", tags = "1, 2")]
	pub clip: Option<Clip>,
}

#[derive(Clone, Copy, Oneof)]
pub enum Clip {
	#[prost(message, tag = "1")]
	Audio(AudioClip),
	#[prost(message, tag = "2")]
	Midi(MidiClip),
}

#[derive(Clone, Copy, Message)]
pub struct AudioClip {
	#[prost(message, required)]
	pub sample: SampleIndex,
	#[prost(message, required)]
	pub position: ClipPosition,
}

#[derive(Clone, Copy, Message)]
pub struct MidiClip {
	#[prost(message, required)]
	pub pattern: PatternIndex,
	#[prost(message, required)]
	pub position: ClipPosition,
}

#[derive(Clone, Copy, Message)]
pub struct NotePosition {
	#[prost(uint32)]
	pub start: u32,
	#[prost(uint32)]
	pub end: u32,
}

#[derive(Clone, Copy, Message)]
pub struct ClipPosition {
	#[prost(message, required)]
	pub position: NotePosition,
	#[prost(uint32)]
	pub offset: u32,
}

#[derive(Message)]
pub struct Plugin {
	#[prost(bytes = "vec")]
	pub id: Vec<u8>,
	#[prost(bytes = "vec", optional)]
	pub state: Option<Vec<u8>>,
	#[prost(float, default = 1.0)]
	pub mix: f32,
	#[prost(bool, default = true)]
	pub enabled: bool,
}

impl Plugin {
	#[must_use]
	pub fn id(&self) -> &CStr {
		CStr::from_bytes_with_nul(&self.id).unwrap()
	}
}

impl From<AudioClip> for OptionClip {
	fn from(value: AudioClip) -> Self {
		Self {
			clip: Some(Clip::Audio(value)),
		}
	}
}

impl From<MidiClip> for OptionClip {
	fn from(value: MidiClip) -> Self {
		Self {
			clip: Some(Clip::Midi(value)),
		}
	}
}
