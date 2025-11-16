use crate::proto;
use prost::Message as _;
use std::io::Cursor;
use yazi::{Format, decompress};

#[derive(Debug)]
pub struct Reader(proto::Project);

impl Reader {
	#[must_use]
	pub fn new(gdp: &[u8]) -> Option<Self> {
		let gdp = decompress(gdp.strip_prefix(b"gdp")?, Format::Raw).ok()?.0;
		proto::Project::decode(&mut Cursor::new(gdp)).map(Self).ok()
	}

	#[must_use]
	pub fn rtstate(&self) -> proto::RtState {
		self.0.rtstate
	}

	pub fn iter_samples(&self) -> impl Iterator<Item = (proto::SampleIndex, &proto::Sample)> {
		(0..)
			.map(|index| proto::SampleIndex { index })
			.zip(&self.0.samples)
	}

	pub fn iter_midi_patterns(
		&self,
	) -> impl Iterator<Item = (proto::MidiPatternIndex, &proto::Pattern)> {
		(0..)
			.map(|index| proto::MidiPatternIndex { index })
			.zip(&self.0.midi_patterns)
	}

	pub fn iter_tracks(
		&self,
	) -> impl Iterator<
		Item = (
			proto::TrackIndex,
			impl Iterator<Item = proto::Clip>,
			&proto::Channel,
		),
	> {
		(0..)
			.map(|index| proto::TrackIndex { index })
			.zip(&self.0.tracks)
			.map(|(index, track)| {
				(
					index,
					track.clips.iter().filter_map(|clip| clip.clip),
					&track.channel,
				)
			})
	}

	pub fn iter_channels(&self) -> impl Iterator<Item = (proto::ChannelIndex, &proto::Channel)> {
		(0..)
			.map(|index| proto::ChannelIndex { index })
			.zip(&self.0.channels)
	}

	pub fn iter_track_to_channel(
		&self,
	) -> impl Iterator<Item = (proto::TrackIndex, proto::ChannelIndex)> {
		(0..)
			.map(|index| proto::TrackIndex { index })
			.zip(&self.0.tracks)
			.flat_map(|(index, track)| {
				track
					.channel
					.connections
					.iter()
					.map(move |&channel| (index, channel))
			})
	}

	pub fn iter_channel_to_channel(
		&self,
	) -> impl Iterator<Item = (proto::ChannelIndex, proto::ChannelIndex)> {
		(0..)
			.map(|index| proto::ChannelIndex { index })
			.zip(&self.0.channels)
			.flat_map(|(index, channel)| {
				channel
					.connections
					.iter()
					.map(move |&channel| (index, channel))
			})
	}
}
