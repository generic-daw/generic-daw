use super::track::Track;
use bit_set::BitSet;
use generic_daw_core::{
    DawCtxMessage, Meter, MixerNode, Producer, Stream, StreamTrait as _,
    audio_graph::{AudioGraph, AudioGraphNodeImpl as _, NodeId},
    build_output_stream,
    oneshot::{self, Receiver},
};
use generic_daw_utils::{HoleyVec, NoDebug};
use hound::WavWriter;
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::Ordering::{AcqRel, Acquire, Release},
    },
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NodeType {
    Master,
    Track,
    Mixer,
}

#[derive(Debug)]
pub struct Arrangement {
    tracks: Vec<Track>,
    nodes: HoleyVec<(Arc<MixerNode>, BitSet, NodeType)>,
    master_node_id: NodeId,

    producer: Producer<DawCtxMessage>,
    stream: NoDebug<Stream>,
    meter: Arc<Meter>,
}

impl Arrangement {
    pub fn new() -> (Self, Arc<Meter>) {
        let (stream, master_node, producer, meter) = build_output_stream(44100, 1024);

        let master_node_id = master_node.id();
        let mut channels = HoleyVec::default();
        channels.insert(
            *master_node_id,
            (master_node, BitSet::default(), NodeType::Master),
        );

        (
            Self {
                tracks: Vec::new(),
                nodes: channels,
                master_node_id,

                producer,
                stream: stream.into(),
                meter: meter.clone(),
            },
            meter,
        )
    }

    pub fn stop(&mut self) {
        self.producer.push(DawCtxMessage::Reset).unwrap();
    }

    pub fn master(&self) -> &(Arc<MixerNode>, BitSet, NodeType) {
        self.node(self.master_node_id)
    }

    pub fn tracks(&self) -> &[Track] {
        &self.tracks
    }

    pub fn channels(&self) -> impl Iterator<Item = &Arc<MixerNode>> {
        self.nodes
            .values()
            .filter_map(|(node, _, ty)| (*ty == NodeType::Mixer).then_some(node))
    }

    pub fn node(&self, id: NodeId) -> &(Arc<MixerNode>, BitSet, NodeType) {
        &self.nodes[*id]
    }

    pub fn add_channel(&mut self) -> (NodeId, Receiver<(NodeId, NodeId)>) {
        let node = Arc::new(MixerNode::default());
        let id = node.id();

        self.nodes
            .insert(*id, (node.clone(), BitSet::default(), NodeType::Mixer));
        self.producer
            .push(DawCtxMessage::Insert(node.into()))
            .unwrap();
        (id, self.request_connect(self.master_node_id, id))
    }

    pub fn add_track(&mut self, track: impl Into<Track>) -> Receiver<(NodeId, NodeId)> {
        let track = track.into();
        let id = track.id();

        self.tracks.push(track.clone());
        self.nodes.insert(
            *id,
            (track.node().clone(), BitSet::default(), NodeType::Track),
        );
        self.producer
            .push(DawCtxMessage::Insert(track.into()))
            .unwrap();
        self.request_connect(self.master_node_id, id)
    }

    pub fn remove(&mut self, track: usize) -> NodeId {
        let id = self.tracks.remove(track).id();
        self.nodes.remove(*id);
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
        self.nodes.get_mut(*to).unwrap().1.insert(*from);
    }

    pub fn disconnect(&mut self, from: NodeId, to: NodeId) {
        self.producer
            .push(DawCtxMessage::Disconnect(from, to))
            .unwrap();
        self.nodes.get_mut(*to).unwrap().1.remove(*from);
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
