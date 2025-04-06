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

    pub fn iter_audios(
        &self,
    ) -> impl Iterator<
        Item = (
            proto::project::track::audio_clip::AudioIndex,
            &proto::project::Audio,
        ),
    > {
        (0..)
            .map(|index| proto::project::track::audio_clip::AudioIndex { index })
            .zip(&self.0.audios)
    }

    pub fn iter_midis(
        &self,
    ) -> impl Iterator<
        Item = (
            proto::project::track::midi_clip::MidiIndex,
            &proto::project::Midi,
        ),
    > {
        (0..)
            .map(|index| proto::project::track::midi_clip::MidiIndex { index })
            .zip(&self.0.midis)
    }

    pub fn iter_tracks(
        &self,
    ) -> impl Iterator<
        Item = (
            proto::project::track::TrackIndex,
            &[proto::project::track::Clip],
            Option<&proto::project::Channel>,
        ),
    > {
        (0..)
            .map(|index| proto::project::track::TrackIndex { index })
            .zip(&self.0.tracks)
            .map(|(index, track)| (index, &*track.clips, track.channel.as_ref()))
    }

    pub fn iter_channels(
        &self,
    ) -> impl Iterator<
        Item = (
            proto::project::channel::ChannelIndex,
            &proto::project::Channel,
        ),
    > {
        (0..)
            .map(|index| proto::project::channel::ChannelIndex { index })
            .zip(&self.0.channels)
    }

    pub fn iter_connections_track_channel(
        &self,
    ) -> impl Iterator<
        Item = (
            proto::project::track::TrackIndex,
            proto::project::channel::ChannelIndex,
        ),
    > {
        (0..)
            .map(|index| proto::project::track::TrackIndex { index })
            .zip(&self.0.tracks)
            .flat_map(|(index, track)| {
                track
                    .channel
                    .as_ref()
                    .map(|channel| &channel.connections)
                    .into_iter()
                    .flatten()
                    .map(move |&channel| (index, channel))
            })
    }

    pub fn iter_connections_channel_channel(
        &self,
    ) -> impl Iterator<
        Item = (
            proto::project::channel::ChannelIndex,
            proto::project::channel::ChannelIndex,
        ),
    > {
        (0..)
            .map(|index| proto::project::channel::ChannelIndex { index })
            .zip(&self.0.channels)
            .flat_map(|(index, channel)| {
                channel
                    .connections
                    .iter()
                    .map(move |&channel| (index, channel))
            })
    }
}
