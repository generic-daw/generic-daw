use super::track::Track;
use generic_daw_core::{
    audio_graph::AudioGraph, DawCtxMessage, Meter, Producer, Stream, StreamTrait as _,
};
use hound::WavWriter;
use rfd::FileHandle;
use std::{
    fmt::{Debug, Formatter},
    path::Path,
    sync::{
        atomic::Ordering::{AcqRel, Release},
        Arc,
    },
};

pub struct Arrangement {
    tracks: Vec<Track>,
    producer: Producer<DawCtxMessage<FileHandle>>,
    stream: Stream,
    pub meter: Arc<Meter>,
}

impl Debug for Arrangement {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Arrangement")
            .field("tracks", &self.tracks)
            .field("producer", &self.producer)
            .field("meter", &self.meter)
            .finish_non_exhaustive()
    }
}

impl Arrangement {
    pub fn new(
        producer: Producer<DawCtxMessage<FileHandle>>,
        stream: Stream,
        meter: Arc<Meter>,
    ) -> Self {
        Self {
            tracks: Vec::new(),
            producer,
            stream,
            meter,
        }
    }

    pub fn tracks(&self) -> &[Track] {
        &self.tracks
    }

    pub fn push(&mut self, track: impl Into<Track>) {
        let track = track.into();
        self.tracks.push(track.clone());

        let id = track.id();
        self.producer
            .push(DawCtxMessage::Insert(track.into()))
            .unwrap();
        self.producer
            .push(DawCtxMessage::ConnectToMaster(id))
            .unwrap();
    }

    pub fn clone_clip(&mut self, track: usize, clip: usize) {
        self.tracks[track].clone_clip(clip);

        self.producer
            .push(DawCtxMessage::Insert(self.tracks[track].clone().into()))
            .unwrap();
    }

    pub fn delete_clip(&mut self, track: usize, clip: usize) {
        self.tracks[track].delete_clip(clip);

        self.producer
            .push(DawCtxMessage::Insert(self.tracks[track].clone().into()))
            .unwrap();
    }

    pub fn clip_switch_track(&mut self, track: usize, clip: usize, new_track: usize) -> bool {
        let inner = self.tracks[track].get_clip(clip);

        if self.tracks[new_track].try_add_clip(inner) {
            self.tracks[track].delete_clip(clip);

            self.producer
                .push(DawCtxMessage::Insert(self.tracks[track].clone().into()))
                .unwrap();
            self.producer
                .push(DawCtxMessage::Insert(self.tracks[new_track].clone().into()))
                .unwrap();

            true
        } else {
            false
        }
    }

    pub fn request_export(&mut self, path: FileHandle) {
        self.producer
            .push(DawCtxMessage::RequestAudioGraph(path))
            .unwrap();
    }

    pub fn export(&mut self, path: &Path, mut audio_graph: AudioGraph) {
        const CHUNK_SIZE: usize = 64;

        self.stream.pause().unwrap();

        let playing = self.meter.playing.swap(true, AcqRel);
        let metronome = self.meter.metronome.swap(false, AcqRel);

        let mut writer = WavWriter::create(
            path,
            hound::WavSpec {
                channels: 2,
                sample_rate: self.meter.sample_rate,
                bits_per_sample: 32,
                sample_format: hound::SampleFormat::Float,
            },
        )
        .unwrap();

        let mut buf = [0.0; CHUNK_SIZE];

        let len = self.tracks.iter().map(Track::len).max().unwrap_or_default();
        let len = len.in_interleaved_samples(&self.meter);

        for i in (0..len).step_by(CHUNK_SIZE) {
            audio_graph.fill_buf(i, &mut buf);

            for s in buf {
                writer.write_sample(s).unwrap();
            }
        }

        writer.finalize().unwrap();

        self.meter.playing.store(playing, Release);
        self.meter.metronome.store(metronome, Release);

        self.producer
            .push(DawCtxMessage::AudioGraph(audio_graph))
            .unwrap();

        self.stream.play().unwrap();
    }
}
