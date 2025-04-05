use crate::{AudioIndex, ChannelIndex, MidiIndex, TrackIndex, proto};
use prost::Message as _;
use std::{marker::PhantomData, path::Path};
use yazi::{CompressionLevel, Format, compress};

#[derive(Debug)]
pub struct Writer<T> {
    inner: proto::Project,
    _state: PhantomData<T>,
}

impl Writer<AudioIndex> {
    #[must_use]
    pub fn new(bpm: u32, numerator: u32) -> Self {
        Self {
            inner: proto::Project {
                meter: Some(proto::project::Meter { bpm, numerator }),
                ..proto::Project::default()
            },
            _state: PhantomData,
        }
    }

    pub fn push_audio(&mut self, path: impl AsRef<Path>) -> AudioIndex {
        self.inner.audios.push(proto::project::Audio {
            components: path
                .as_ref()
                .components()
                .map(|component| component.as_os_str().to_string_lossy().to_string())
                .collect(),
        });

        AudioIndex {
            index: self.inner.audios.len() as u32 - 1,
        }
    }

    #[must_use]
    pub fn next(self) -> Writer<MidiIndex> {
        Writer {
            inner: self.inner,
            _state: PhantomData,
        }
    }
}

impl Writer<MidiIndex> {
    pub fn push_midi(
        &mut self,
        notes: impl IntoIterator<Item = proto::project::midi::Note>,
    ) -> MidiIndex {
        self.inner.midis.push(proto::project::Midi {
            notes: notes.into_iter().collect(),
        });

        MidiIndex {
            index: self.inner.midis.len() as u32 - 1,
        }
    }

    #[must_use]
    pub fn next(self) -> Writer<TrackIndex> {
        Writer {
            inner: self.inner,
            _state: PhantomData,
        }
    }
}

impl Writer<TrackIndex> {
    pub fn push_track(
        &mut self,
        clips: impl IntoIterator<Item = proto::project::track::Clip>,
        plugins: impl IntoIterator<Item = proto::project::channel::Plugin>,
    ) -> TrackIndex {
        self.inner.tracks.push(proto::project::Track {
            clips: clips.into_iter().collect(),
            channel: Some(proto::project::Channel {
                connections: Vec::new(),
                plugins: plugins.into_iter().collect(),
            }),
        });

        TrackIndex {
            index: self.inner.tracks.len() as u32 - 1,
        }
    }

    #[must_use]
    pub fn next(self) -> Writer<ChannelIndex> {
        Writer {
            inner: self.inner,
            _state: PhantomData,
        }
    }
}

impl Writer<ChannelIndex> {
    #[must_use]
    pub fn push_channel(
        &mut self,
        plugins: impl IntoIterator<Item = proto::project::channel::Plugin>,
    ) -> ChannelIndex {
        self.inner.channels.push(proto::project::Channel {
            connections: Vec::new(),
            plugins: plugins.into_iter().collect(),
        });

        ChannelIndex {
            index: self.inner.channels.len() as u32 - 1,
        }
    }

    #[must_use]
    pub fn next(self) -> Writer<()> {
        Writer {
            inner: self.inner,
            _state: PhantomData,
        }
    }
}

impl Writer<()> {
    pub fn connect_track_to_channel(&mut self, from: TrackIndex, to: ChannelIndex) {
        self.inner.tracks[from.index as usize]
            .channel
            .as_mut()
            .unwrap()
            .connections
            .push(to);
    }

    pub fn connect_channel_to_channel(&mut self, from: ChannelIndex, to: ChannelIndex) {
        self.inner.channels[from.index as usize]
            .connections
            .push(to);
    }

    #[must_use]
    pub fn finalize(self) -> Vec<u8> {
        let mut pbf = Vec::new();
        self.inner.encode(&mut pbf).unwrap();
        compress(&pbf, Format::Raw, CompressionLevel::BestSize).unwrap()
    }
}
