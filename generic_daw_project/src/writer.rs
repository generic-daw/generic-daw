use crate::proto;
use prost::Message as _;
use std::path::Path;
use yazi::{CompressionLevel, Format, compress};

#[derive(Debug)]
pub struct Writer(proto::Project);

impl Writer {
    #[must_use]
    pub fn new(bpm: u32, numerator: u32) -> Self {
        Self(proto::Project {
            meter: Some(proto::project::Meter { bpm, numerator }),
            ..proto::Project::default()
        })
    }

    pub fn push_audio(
        &mut self,
        path: impl AsRef<Path>,
    ) -> proto::project::track::audio_clip::AudioIndex {
        self.0.audios.push(proto::project::Audio {
            components: path
                .as_ref()
                .components()
                .map(|component| component.as_os_str().to_string_lossy().to_string())
                .collect(),
        });

        proto::project::track::audio_clip::AudioIndex {
            index: self.0.audios.len() as u32 - 1,
        }
    }

    pub fn push_midi(
        &mut self,
        notes: impl IntoIterator<Item = proto::project::midi::Note>,
    ) -> proto::project::track::midi_clip::MidiIndex {
        self.0.midis.push(proto::project::Midi {
            notes: notes.into_iter().collect(),
        });

        proto::project::track::midi_clip::MidiIndex {
            index: self.0.midis.len() as u32 - 1,
        }
    }

    pub fn push_track(
        &mut self,
        clips: impl IntoIterator<Item = proto::project::track::Clip>,
        plugins: impl IntoIterator<Item = proto::project::channel::Plugin>,
    ) -> proto::project::track::TrackIndex {
        self.0.tracks.push(proto::project::Track {
            clips: clips.into_iter().collect(),
            channel: Some(proto::project::Channel {
                connections: Vec::new(),
                plugins: plugins.into_iter().collect(),
            }),
        });

        proto::project::track::TrackIndex {
            index: self.0.tracks.len() as u32 - 1,
        }
    }

    #[must_use]
    pub fn push_channel(
        &mut self,
        plugins: impl IntoIterator<Item = proto::project::channel::Plugin>,
    ) -> proto::project::channel::ChannelIndex {
        self.0.channels.push(proto::project::Channel {
            connections: Vec::new(),
            plugins: plugins.into_iter().collect(),
        });

        proto::project::channel::ChannelIndex {
            index: self.0.channels.len() as u32 - 1,
        }
    }

    pub fn connect_track_to_channel(
        &mut self,
        from: proto::project::track::TrackIndex,
        to: proto::project::channel::ChannelIndex,
    ) {
        self.0.tracks[from.index as usize]
            .channel
            .as_mut()
            .unwrap()
            .connections
            .push(to);
    }

    pub fn connect_channel_to_channel(
        &mut self,
        from: proto::project::channel::ChannelIndex,
        to: proto::project::channel::ChannelIndex,
    ) {
        self.0.channels[from.index as usize].connections.push(to);
    }

    #[must_use]
    pub fn finalize(self) -> Vec<u8> {
        let mut pbf = Vec::new();
        self.0.encode(&mut pbf).unwrap();
        compress(&pbf, Format::Raw, CompressionLevel::BestSize).unwrap()
    }
}
