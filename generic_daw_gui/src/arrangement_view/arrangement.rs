use super::{TrackClipWrapper, track::Track};
use bit_set::BitSet;
use generic_daw_core::{
    DawCtxMessage, Meter, MixerNode, Stream, StreamTrait as _,
    audio_graph::{AudioGraphNodeImpl as _, NodeId},
    build_output_stream, export,
};
use generic_daw_utils::{HoleyVec, NoDebug};
use oneshot::Receiver;
use rtrb::Producer;
use std::{path::Path, sync::Arc};

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
    pub fn create() -> (Self, Arc<Meter>) {
        let (stream, master_node, producer, meter) = build_output_stream(44100, 1024);
        let master_node_id = master_node.id();
        let mut channels = HoleyVec::default();
        channels.insert(
            master_node_id.get(),
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

    pub fn track_of(&self, id: NodeId) -> Option<usize> {
        self.tracks.iter().position(|t| t.id() == id)
    }

    pub fn channels(&self) -> impl Iterator<Item = &Arc<MixerNode>> {
        self.nodes
            .values()
            .filter_map(|(node, _, ty)| (*ty == NodeType::Mixer).then_some(node))
    }

    pub fn node(&self, id: NodeId) -> &(Arc<MixerNode>, BitSet, NodeType) {
        &self.nodes[id.get()]
    }

    pub fn add_channel(&mut self) -> Receiver<(NodeId, NodeId)> {
        let node = Arc::new(MixerNode::default());
        let id = node.id();

        self.nodes
            .insert(id.get(), (node.clone(), BitSet::default(), NodeType::Mixer));
        self.producer
            .push(DawCtxMessage::Insert(node.into()))
            .unwrap();
        self.request_connect(self.master_node_id, id)
    }

    pub fn remove_channel(&mut self, id: NodeId) {
        self.nodes.remove(id.get());
        self.producer.push(DawCtxMessage::Remove(id)).unwrap();
    }

    pub fn add_track(&mut self, track: impl Into<Track>) -> Receiver<(NodeId, NodeId)> {
        let track = track.into();
        let id = track.id();

        self.tracks.push(track.clone());
        self.nodes.insert(
            id.get(),
            (track.node().clone(), BitSet::default(), NodeType::Track),
        );
        self.producer
            .push(DawCtxMessage::Insert(track.into()))
            .unwrap();
        self.request_connect(self.master_node_id, id)
    }

    pub fn remove_track(&mut self, id: NodeId) {
        let idx = self
            .tracks
            .iter()
            .position(|track| track.id() == id)
            .unwrap();

        self.tracks.remove(idx);
    }

    pub fn request_connect(&mut self, from: NodeId, to: NodeId) -> Receiver<(NodeId, NodeId)> {
        let (sender, receiver) = oneshot::channel();
        self.producer
            .push(DawCtxMessage::Connect(from, to, sender))
            .unwrap();

        receiver
    }

    pub fn connect_succeeded(&mut self, from: NodeId, to: NodeId) {
        self.nodes.get_mut(to.get()).unwrap().1.insert(from.get());
    }

    pub fn disconnect(&mut self, from: NodeId, to: NodeId) {
        self.producer
            .push(DawCtxMessage::Disconnect(from, to))
            .unwrap();
        self.nodes.get_mut(to.get()).unwrap().1.remove(from.get());
    }

    pub fn add_clip(&mut self, track: usize, clip: impl Into<TrackClipWrapper>) {
        match &mut self.tracks[track] {
            Track::AudioTrack(track) => track.clips.push(clip.into().try_into().unwrap()),
            Track::MidiTrack(track) => track.clips.push(clip.into().try_into().unwrap()),
        }

        self.producer
            .push(DawCtxMessage::Insert(self.tracks[track].clone().into()))
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

    pub fn export(&mut self, path: &Path) {
        let (sender, receiver) = oneshot::channel();

        self.producer
            .push(DawCtxMessage::RequestAudioGraph(sender))
            .unwrap();

        let mut audio_graph = receiver.recv().unwrap();

        self.stream.pause().unwrap();

        export(
            &mut audio_graph,
            path,
            &self.meter,
            self.tracks()
                .iter()
                .map(Track::len)
                .max()
                .unwrap_or_default(),
        );

        self.producer
            .push(DawCtxMessage::AudioGraph(audio_graph))
            .unwrap();

        self.stream.play().unwrap();
    }
}
