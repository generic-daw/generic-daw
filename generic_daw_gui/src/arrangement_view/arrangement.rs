use super::track::Track;
use bit_set::BitSet;
use generic_daw_core::{
    DawCtxMessage, Meter, MixerNode, Producer, Stream, StreamTrait as _,
    audio_graph::{AudioGraph, AudioGraphNodeImpl as _, NodeId},
    build_output_stream,
    oneshot::{self, Receiver},
};
use generic_daw_utils::HoleyVec;
use hound::WavWriter;
use std::{
    fmt::{Debug, Formatter},
    path::Path,
    sync::{
        Arc,
        atomic::Ordering::{AcqRel, Acquire, Release},
    },
};

pub struct Arrangement {
    tracks: Vec<Track>,
    channels: HoleyVec<(Arc<MixerNode>, BitSet)>,
    master_node_id: NodeId,

    producer: Producer<DawCtxMessage>,
    stream: Stream,
    meter: Arc<Meter>,
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
    pub fn new() -> (Self, Arc<Meter>) {
        let (stream, master_node, producer, meter) = build_output_stream(44100, 1024);

        let master_node_id = master_node.id();
        let mut channels = HoleyVec::default();
        channels.insert(*master_node_id, (master_node, BitSet::default()));

        (
            Self {
                tracks: Vec::new(),
                channels,
                master_node_id,

                producer,
                stream,
                meter: meter.clone(),
            },
            meter,
        )
    }

    pub fn stop(&mut self) {
        self.producer.push(DawCtxMessage::Reset).unwrap();
    }

    pub fn tracks(&self) -> &[Track] {
        &self.tracks
    }

    pub fn node(&self, id: NodeId) -> &MixerNode {
        &self.channel(id).0
    }

    pub fn channels(&self) -> impl Iterator<Item = (usize, &(Arc<MixerNode>, BitSet))> {
        self.channels.iter()
    }

    pub fn channel(&self, id: NodeId) -> &(Arc<MixerNode>, BitSet) {
        &self.channels[*id]
    }

    #[must_use]
    pub fn push(&mut self, track: impl Into<Track>) -> Receiver<(NodeId, NodeId)> {
        let track = track.into();
        let id = track.id();
        let node = track.node().clone();

        self.tracks.push(track.clone());
        self.channels.insert(*id, (node, BitSet::default()));

        self.producer
            .push(DawCtxMessage::Insert(track.into()))
            .unwrap();
        self.request_connect(self.master_node_id, id)
    }

    pub fn remove(&mut self, track: usize) -> NodeId {
        let id = self.tracks.remove(track).id();
        self.channels.remove(*id);
        self.producer.push(DawCtxMessage::Remove(id)).unwrap();
        id
    }

    pub fn request_connect(&mut self, from: NodeId, to: NodeId) -> Receiver<(NodeId, NodeId)> {
        let (sender, receiver) = oneshot::channel();
        self.producer
            .push(DawCtxMessage::Connect(from, to, sender))
            .unwrap();

        receiver
    }

    pub fn connect_succeeded(&mut self, from: NodeId, to: NodeId) {
        self.channels.get_mut(*to).unwrap().1.insert(*from);
    }

    pub fn disconnect(&mut self, from: NodeId, to: NodeId) {
        self.producer
            .push(DawCtxMessage::Disconnect(from, to))
            .unwrap();
        self.channels.get_mut(*to).unwrap().1.remove(*from);
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

    pub fn request_export(&mut self) -> Receiver<AudioGraph> {
        let (sender, receiver) = oneshot::channel();

        self.producer
            .push(DawCtxMessage::RequestAudioGraph(sender))
            .unwrap();

        receiver
    }

    pub fn export(&mut self, mut audio_graph: AudioGraph, path: &Path) {
        const CHUNK_SIZE: usize = 64;

        self.stream.pause().unwrap();

        let playing = self.meter.playing.swap(true, AcqRel);
        let metronome = self.meter.metronome.swap(false, AcqRel);
        let sample = self.meter.sample.load(Acquire);

        audio_graph.reset();

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
        let len = len.in_interleaved_samples(self.meter.bpm.load(Acquire), self.meter.sample_rate);

        for i in (0..len).step_by(CHUNK_SIZE) {
            self.meter.sample.store(i, Release);

            audio_graph.fill_buf(&mut buf);

            for s in buf {
                writer.write_sample(s).unwrap();
            }
        }

        writer.finalize().unwrap();

        self.meter.playing.store(playing, Release);
        self.meter.metronome.store(metronome, Release);
        self.meter.sample.store(sample, Release);

        self.producer
            .push(DawCtxMessage::AudioGraph(audio_graph))
            .unwrap();

        self.stream.play().unwrap();
    }
}
