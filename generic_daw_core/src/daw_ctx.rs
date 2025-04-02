use crate::{AudioGraph, AudioGraphNode, Master, Meter, MixerNode};
use audio_graph::NodeId;
use log::trace;
use oneshot::Sender;
use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::Arc;

#[derive(Debug)]
pub enum DawCtxMessage {
    Insert(AudioGraphNode),
    Remove(NodeId),
    Connect(NodeId, NodeId, Sender<(NodeId, NodeId)>),
    Disconnect(NodeId, NodeId),
    RequestAudioGraph(Sender<AudioGraph>),
    Reset,
    AudioGraph(AudioGraph),
}

pub struct DawCtx {
    pub meter: Arc<Meter>,
    audio_graph: AudioGraph,
    consumer: Consumer<DawCtxMessage>,
}

impl DawCtx {
    pub fn create(
        sample_rate: u32,
        buffer_size: u32,
    ) -> (Self, Arc<MixerNode>, Producer<DawCtxMessage>) {
        let (ui_producer, consumer) = RingBuffer::new(16);

        let meter = Arc::new(Meter::new(sample_rate, buffer_size));
        let node = Arc::<MixerNode>::default();
        let master = Master::new(meter.clone(), node.clone());
        let audio_graph = AudioGraph::new(master.into());

        let audio_ctx = Self {
            meter,
            audio_graph,
            consumer,
        };

        (audio_ctx, node, ui_producer)
    }

    pub fn process(&mut self, buf: &mut [f32]) {
        while let Ok(msg) = self.consumer.pop() {
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
    }
}
