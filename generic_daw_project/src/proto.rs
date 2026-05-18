use prost::{Message, Oneof};
use std::ffi::CStr;

#[derive(Clone, Copy, Eq, Hash, Message, PartialEq)]
pub struct SampleIndex {
	#[prost(uint32)]
	pub(crate) index: u32,
}

#[derive(Clone, Copy, Eq, Hash, Message, PartialEq)]
pub struct MidiPatternIndex {
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

#[derive(Clone, Message)]
pub struct Project {
	#[prost(message, required)]
	pub transport: Transport,
	#[prost(message, repeated)]
	pub samples: Vec<Sample>,
	#[prost(message, repeated)]
	pub midi_patterns: Vec<MidiPattern>,
	#[prost(message, repeated)]
	pub tracks: Vec<Track>,
	#[prost(message, repeated)]
	pub channels: Vec<Channel>,
	#[prost(message)]
	pub view: Option<ViewState>,
}

#[derive(Clone, Copy, Message)]
pub struct Transport {
	#[prost(uint32)]
	pub bpm: u32,
	#[prost(uint32)]
	pub numerator: u32,
	#[prost(message)]
	pub loop_range: Option<BeatRange>,
}

#[derive(Clone, Message)]
pub struct Sample {
	#[prost(string)]
	pub name: String,
	#[prost(uint32)]
	pub crc: u32,
	#[prost(uint64)]
	pub len: u64,
}

#[derive(Clone, Message)]
pub struct MidiPattern {
	#[prost(message, repeated)]
	pub notes: Vec<Note>,
	#[prost(string)]
	pub name: String,
}

#[derive(Clone, Copy, Message)]
pub struct Note {
	#[prost(uint32)]
	pub key: u32,
	#[prost(float, default = 1.0)]
	pub velocity: f32,
	#[prost(message, required)]
	pub position: BeatRange,
}

#[derive(Clone, Message)]
pub struct Track {
	#[prost(message, repeated)]
	pub clips: Vec<OptionClip>,
	#[prost(message, required)]
	pub channel: Channel,
}

#[derive(Clone, Message)]
pub struct Channel {
	#[prost(message, repeated)]
	pub connections: Vec<Connection>,
	#[prost(message, repeated)]
	pub plugins: Vec<Plugin>,
	#[prost(float, default = 1.0)]
	pub volume: f32,
	#[prost(message, required)]
	pub pan: OptionPanMode,
	#[prost(bool, default = true)]
	pub enabled: bool,
	#[prost(bool, default = false)]
	pub bypassed: bool,
}

#[derive(Clone, Copy, Message, PartialEq)]
pub struct Connection {
	#[prost(uint32)]
	pub index: u32,
	#[prost(float, default = 1.0)]
	pub mix: f32,
}

#[derive(Clone, Message)]
pub struct Plugin {
	#[prost(bytes = "vec")]
	pub id: Vec<u8>,
	#[prost(bytes = "vec", optional)]
	pub state: Option<Vec<u8>>,
	#[prost(float, default = 1.0)]
	pub mix: f32,
	#[prost(bool, default = true)]
	pub active: bool,
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
	#[prost(message)]
	pub position_compat: Option<OffsetBeatRange>,
	#[prost(float, optional)]
	pub stretch_compat: Option<f32>,
	#[prost(message, required)]
	pub position: OffsetBeatSpan,
	#[prost(double, default = 1.0)]
	pub stretch: f64,
}

#[derive(Clone, Copy, Message)]
pub struct MidiClip {
	#[prost(message, required)]
	pub pattern: MidiPatternIndex,
	#[prost(message, required)]
	pub position: OffsetBeatRange,
}

#[derive(Clone, Copy, Message)]
pub struct BeatRange {
	#[prost(uint64)]
	pub start: u64,
	#[prost(uint64)]
	pub end: u64,
}

#[derive(Clone, Copy, Message)]
pub struct OffsetBeatRange {
	#[prost(message, required)]
	pub position: BeatRange,
	#[prost(uint64)]
	pub offset: u64,
}

#[derive(Clone, Copy, Message)]
pub struct BeatSpan {
	#[prost(uint64)]
	pub start: u64,
	#[prost(uint64)]
	pub len: u64,
}

#[derive(Clone, Copy, Message)]
pub struct OffsetBeatSpan {
	#[prost(message, required)]
	pub position: BeatSpan,
	#[prost(uint64)]
	pub offset: u64,
}

#[derive(Clone, Copy, Message)]
pub struct ViewState {
	#[prost(message, required)]
	pub playlist: TabState,
	#[prost(message, required)]
	pub piano_roll: TabState,
}

#[derive(Clone, Copy, Message)]
pub struct TabState {
	#[prost(message, required)]
	pub position: Vector,
	#[prost(message, required)]
	pub scale: Vector,
}

#[derive(Clone, Copy, Message)]
pub struct Vector {
	#[prost(float)]
	pub x: f32,
	#[prost(float)]
	pub y: f32,
}

impl Plugin {
	#[must_use]
	pub fn id(&self) -> &CStr {
		CStr::from_bytes_with_nul(&self.id).unwrap()
	}
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
