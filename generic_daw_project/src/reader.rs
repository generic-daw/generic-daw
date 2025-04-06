use crate::proto;
use prost::Message as _;
use std::io::Cursor;
use yazi::{Format, decompress};

#[derive(Debug)]
pub struct Reader(proto::Project);

impl Reader {
    pub fn new(pbf: &[u8]) -> Option<Self> {
        let pbf = decompress(pbf, Format::Zlib).ok()?.0;
        proto::Project::decode(&mut Cursor::new(pbf)).map(Self).ok()
    }

    pub fn iter_audios(
        &self,
    ) -> impl Iterator<
        Item = (
            proto::project::track::audio_clip::AudioIndex,
            &proto::project::Audio,
        ),
    > {
        self.0.audios.iter().zip(0..).map(|(audio, index)| {
            (
                proto::project::track::audio_clip::AudioIndex { index },
                audio,
            )
        })
    }

    pub fn iter_midis(
        &self,
    ) -> impl Iterator<
        Item = (
            proto::project::track::midi_clip::MidiIndex,
            &proto::project::Midi,
        ),
    > {
        self.0
            .midis
            .iter()
            .zip(0..)
            .map(|(midi, index)| (proto::project::track::midi_clip::MidiIndex { index }, midi))
    }

    pub fn iter_tracks(
        &self,
    ) -> impl Iterator<
        Item = (
            proto::project::track::TrackIndex,
            &[proto::project::track::Clip],
            &[proto::project::channel::Plugin],
        ),
    > {
        self.0.tracks.iter().zip(0..).map(|(track, index)| {
            (
                proto::project::track::TrackIndex { index },
                &*track.clips,
                &*track.channel.as_ref().unwrap().plugins,
            )
        })
    }

    pub fn iter_channels(
        &self,
    ) -> impl Iterator<
        Item = (
            proto::project::channel::ChannelIndex,
            &[proto::project::channel::Plugin],
        ),
    > {
        self.0.channels.iter().zip(0..).map(|(channel, index)| {
            (
                proto::project::channel::ChannelIndex { index },
                &*channel.plugins,
            )
        })
    }

    pub fn iter_connections_track_channel(
        &self,
    ) -> impl Iterator<
        Item = (
            proto::project::track::TrackIndex,
            proto::project::channel::ChannelIndex,
        ),
    > {
        self.0.tracks.iter().zip(0..).flat_map(|(track, index)| {
            let index = proto::project::track::TrackIndex { index };
            track
                .channel
                .as_ref()
                .unwrap()
                .connections
                .iter()
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
        self.0
            .channels
            .iter()
            .zip(0..)
            .flat_map(|(channel, index)| {
                let index = proto::project::channel::ChannelIndex { index };
                channel
                    .connections
                    .iter()
                    .map(move |&channel| (index, channel))
            })
    }
}
