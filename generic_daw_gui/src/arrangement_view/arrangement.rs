use bit_set::BitSet;
use generic_daw_core::{
    Clip, DawCtxMessage, Meter, MeterDiff, MixerNode, Stream, StreamTrait as _, Track,
    audio_graph::{NodeId, NodeImpl as _},
    build_output_stream, export,
};
use generic_daw_utils::{HoleyVec, NoDebug};
use oneshot::Receiver;
use std::{path::Path, sync::Arc};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NodeType {
    Master,
    Track,
    Mixer,
}

#[derive(Debug)]
pub struct Arrangement {
    meter: Meter,

    tracks: Vec<Track>,
    nodes: HoleyVec<(Arc<MixerNode>, BitSet, NodeType)>,
    master_node_id: NodeId,

    producer: smol::channel::Sender<DawCtxMessage>,
    stream: NoDebug<Stream>,
}

impl Arrangement {
    pub fn create() -> (Self, smol::channel::Receiver<MeterDiff>) {
        let meter = Meter {
            sample_rate: 44100,
            buffer_size: 1024,
            bpm: 140,
            ..Meter::default()
        };

        let (stream, master_node, producer, receiver) = build_output_stream(meter);
        let master_node_id = master_node.id();
        let mut channels = HoleyVec::default();
        channels.insert(
            master_node_id.get(),
            (master_node, BitSet::default(), NodeType::Master),
        );

        (
            Self {
                meter,

                tracks: Vec::new(),
                nodes: channels,
                master_node_id,

                producer,
                stream: stream.into(),
            },
            receiver,
        )
    }

    pub fn meter(&self) -> Meter {
        self.meter
    }

    pub fn modify_meter(&mut self, f: impl FnOnce(&mut Meter)) {
        let mut meter = self.meter;
        f(&mut meter);
        self.producer
            .try_send(DawCtxMessage::MeterDiff(self.meter.diff(meter)))
            .unwrap();
        self.meter = meter;
    }

    pub fn apply_meter_diff(&mut self, diff: MeterDiff) {
        self.meter = self.meter.resolve(diff);
    }

    pub fn stop(&self) {
        self.producer.try_send(DawCtxMessage::Reset).unwrap();
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
            .try_send(DawCtxMessage::Insert(node.into()))
            .unwrap();

        self.request_connect(self.master_node_id, id)
    }

    pub fn remove_channel(&mut self, id: NodeId) {
        self.nodes.remove(id.get());

        self.producer.try_send(DawCtxMessage::Remove(id)).unwrap();
    }

    pub fn add_track(&mut self, track: Track) -> Receiver<(NodeId, NodeId)> {
        let id = track.id();

        self.tracks.push(track.clone());
        self.nodes.insert(
            id.get(),
            (track.node.clone(), BitSet::default(), NodeType::Track),
        );

        self.producer
            .try_send(DawCtxMessage::Insert(track.into()))
            .unwrap();

        self.request_connect(self.master_node_id, id)
    }

    pub fn remove_track(&mut self, idx: usize) {
        self.tracks.remove(idx);
    }

    pub fn request_connect(&self, from: NodeId, to: NodeId) -> Receiver<(NodeId, NodeId)> {
        let (sender, receiver) = oneshot::channel();

        self.producer
            .try_send(DawCtxMessage::Connect(from, to, sender))
            .unwrap();

        receiver
    }

    pub fn connect_succeeded(&mut self, from: NodeId, to: NodeId) {
        self.nodes.get_mut(to.get()).unwrap().1.insert(from.get());
    }

    pub fn disconnect(&mut self, from: NodeId, to: NodeId) {
        self.nodes.get_mut(to.get()).unwrap().1.remove(from.get());

        self.producer
            .try_send(DawCtxMessage::Disconnect(from, to))
            .unwrap();
    }

    pub fn add_clip(&mut self, track: usize, clip: impl Into<Clip>) {
        self.tracks[track].clips.push(clip.into());

        self.producer
            .try_send(DawCtxMessage::Insert(self.tracks[track].clone().into()))
            .unwrap();
    }

    pub fn clone_clip(&mut self, track: usize, clip: usize) {
        let clip = self.tracks[track].clips[clip].clone();
        self.tracks[track].clips.push(clip);

        self.producer
            .try_send(DawCtxMessage::Insert(self.tracks[track].clone().into()))
            .unwrap();
    }

    pub fn delete_clip(&mut self, track: usize, clip: usize) {
        self.tracks[track].clips.remove(clip);

        self.producer
            .try_send(DawCtxMessage::Insert(self.tracks[track].clone().into()))
            .unwrap();
    }

    pub fn clip_switch_track(&mut self, track: usize, clip: usize, new_track: usize) {
        let clip = self.tracks[track].clips.remove(clip);
        self.tracks[new_track].clips.push(clip);

        self.producer
            .try_send(DawCtxMessage::Insert(self.tracks[track].clone().into()))
            .unwrap();
        self.producer
            .try_send(DawCtxMessage::Insert(self.tracks[new_track].clone().into()))
            .unwrap();
    }

    pub fn export(&self, path: &Path) {
        let (sender, receiver) = oneshot::channel();

        self.producer
            .try_send(DawCtxMessage::RequestAudioGraph(sender))
            .unwrap();

        let mut audio_graph = receiver.recv().unwrap();

        self.stream.pause().unwrap();

        export(
            &mut audio_graph,
            path,
            self.tracks()
                .iter()
                .map(Track::len)
                .max()
                .unwrap_or_default(),
        );

        self.producer
            .try_send(DawCtxMessage::AudioGraph(audio_graph))
            .unwrap();

        self.stream.play().unwrap();
    }
}
