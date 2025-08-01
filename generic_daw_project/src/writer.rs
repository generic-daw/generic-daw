use crate::proto;
use prost::Message as _;
use yazi::{CompressionLevel, Format, compress};

#[derive(Debug)]
pub struct Writer(proto::Project);

impl Writer {
	#[must_use]
	pub fn new(bpm: u32, numerator: u32) -> Self {
		Self(proto::Project {
			rtstate: proto::RtState { bpm, numerator },
			..proto::Project::default()
		})
	}

	#[must_use]
	pub fn push_audio(&mut self, name: impl AsRef<str>, hash: u64) -> proto::AudioIndex {
		self.0.audios.push(proto::Audio {
			name: name.as_ref().to_owned(),
			hash,
		});

		proto::AudioIndex {
			index: self.0.audios.len() as u32 - 1,
		}
	}

	#[must_use]
	pub fn push_midi(&mut self, notes: impl IntoIterator<Item = proto::Note>) -> proto::MidiIndex {
		self.0.midis.push(proto::Midi {
			notes: notes.into_iter().collect(),
		});

		proto::MidiIndex {
			index: self.0.midis.len() as u32 - 1,
		}
	}

	#[must_use]
	pub fn push_track(
		&mut self,
		clips: impl IntoIterator<Item = proto::OptionClip>,
		plugins: impl IntoIterator<Item = proto::Plugin>,
		volume: f32,
		pan: f32,
	) -> proto::TrackIndex {
		self.0.tracks.push(proto::Track {
			clips: clips.into_iter().collect(),
			channel: proto::Channel {
				connections: Vec::new(),
				plugins: plugins.into_iter().collect(),
				volume,
				pan,
			},
		});

		proto::TrackIndex {
			index: self.0.tracks.len() as u32 - 1,
		}
	}

	#[must_use]
	pub fn push_channel(
		&mut self,
		plugins: impl IntoIterator<Item = proto::Plugin>,
		volume: f32,
		pan: f32,
	) -> proto::ChannelIndex {
		self.0.channels.push(proto::Channel {
			connections: Vec::new(),
			plugins: plugins.into_iter().collect(),
			volume,
			pan,
		});

		proto::ChannelIndex {
			index: self.0.channels.len() as u32 - 1,
		}
	}

	pub fn connect_track_to_channel(&mut self, from: proto::TrackIndex, to: proto::ChannelIndex) {
		self.0.tracks[from.index as usize]
			.channel
			.connections
			.push(to);
	}

	pub fn connect_channel_to_channel(
		&mut self,
		from: proto::ChannelIndex,
		to: proto::ChannelIndex,
	) {
		self.0.channels[from.index as usize].connections.push(to);
	}

	#[must_use]
	pub fn finalize(self) -> Vec<u8> {
		let mut gdp = Vec::new();
		self.0.encode(&mut gdp).unwrap();
		let mut gdp = compress(&gdp, Format::Raw, CompressionLevel::BestSize).unwrap();
		gdp.splice(0..0, *b"gdp");
		gdp
	}
}
