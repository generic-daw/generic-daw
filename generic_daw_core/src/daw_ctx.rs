use crate::{AudioGraph, AudioGraphNode, Master, Meter, MixerNode};
use async_channel::{Receiver, Sender};
use audio_graph::NodeId;
use log::trace;
use std::sync::{
    Arc,
    atomic::Ordering::{AcqRel, Acquire},
};

#[derive(Debug)]
pub enum DawCtxMessage {
    Insert(AudioGraphNode),
    Remove(NodeId),
    Connect(NodeId, NodeId, oneshot::Sender<(NodeId, NodeId)>),
    Disconnect(NodeId, NodeId),
    RequestAudioGraph(oneshot::Sender<AudioGraph>),
    Reset,
    AudioGraph(AudioGraph),
}

pub struct DawCtx {
    pub meter: Arc<Meter>,
    audio_graph: AudioGraph,
    receiver: Receiver<DawCtxMessage>,
}

impl DawCtx {
    pub fn create(
        sample_rate: u32,
        buffer_size: u32,
    ) -> (Self, Arc<MixerNode>, Sender<DawCtxMessage>) {
        let (ui_producer, consumer) = async_channel::unbounded();

        let meter = Arc::new(Meter::new(sample_rate, buffer_size));
        let node = Arc::<MixerNode>::default();
        let master = Master::new(meter.clone(), node.clone());
        let audio_graph = AudioGraph::new(master.into());

        let audio_ctx = Self {
            meter,
            audio_graph,
            receiver: consumer,
        };

        (audio_ctx, node, ui_producer)
    }

    pub fn process(&mut self, buf: &mut [f32]) {
        while let Ok(msg) = self.receiver.try_recv() {
            trace!("{msg:?}");

            match msg {
                DawCtxMessage::Insert(node) => self.audio_graph.insert(node),
                DawCtxMessage::Remove(node) => self.audio_graph.remove(node),
                DawCtxMessage::Connect(from, to, sender) => {
                    if self.audio_graph.connect(from, to) {
                        sender.send((from, to)).unwrap();
                    }
                }
                DawCtxMessage::Disconnect(from, to) => self.audio_graph.disconnect(from, to),
                DawCtxMessage::RequestAudioGraph(sender) => {
                    let mut audio_graph = AudioGraph::new(Arc::new(MixerNode::default()).into());
                    std::mem::swap(&mut self.audio_graph, &mut audio_graph);
                    sender.send(audio_graph).unwrap();
                }
                DawCtxMessage::Reset => self.audio_graph.reset(),
                DawCtxMessage::AudioGraph(audio_graph) => self.audio_graph = audio_graph,
            }
        }

        self.audio_graph.process(buf);

        for s in &mut *buf {
            *s = s.clamp(-1.0, 1.0);
        }

        if self.meter.playing.load(Acquire) {
            self.meter.sample.fetch_add(buf.len(), AcqRel);
        }
    }
}
