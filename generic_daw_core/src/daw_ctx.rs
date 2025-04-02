use crate::{AudioGraph, AudioGraphNode, METER, Master, Meter, MixerNode, meter::MeterDiff};
use async_channel::{Receiver, Sender};
use audio_graph::NodeId;
use log::trace;
use std::sync::Arc;

#[derive(Debug)]
pub enum DawCtxMessage {
    Insert(AudioGraphNode),
    Remove(NodeId),
    Connect(NodeId, NodeId, oneshot::Sender<(NodeId, NodeId)>),
    Disconnect(NodeId, NodeId),
    RequestAudioGraph(oneshot::Sender<AudioGraph>),
    Reset,
    AudioGraph(AudioGraph),
    MeterDiff(MeterDiff),
}

pub struct DawCtx {
    last_meter: Arc<Meter>,
    audio_graph: AudioGraph,
    sender: Sender<MeterDiff>,
    consumer: Receiver<DawCtxMessage>,
}

impl DawCtx {
    pub fn create(
        meter: Meter,
    ) -> (
        Self,
        Arc<MixerNode>,
        Sender<DawCtxMessage>,
        Receiver<MeterDiff>,
    ) {
        let last_meter = Arc::new(meter);
        let meter = Arc::new(meter);
        METER.store(meter);

        let (ui_producer, consumer) = async_channel::unbounded();
        let (sender, ui_receiver) = async_channel::unbounded();

        let node = Arc::<MixerNode>::default();
        let master = Master::new(node.clone());
        let audio_graph = AudioGraph::new(master.into());

        let audio_ctx = Self {
            last_meter,
            audio_graph,
            sender,
            consumer,
        };

        (audio_ctx, node, ui_producer, ui_receiver)
    }

    pub fn process(&mut self, buf: &mut [f32]) {
        while let Ok(msg) = self.consumer.try_recv() {
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
                DawCtxMessage::MeterDiff(diff) => {
                    let meter = self.last_meter.resolve(diff);

                    *Arc::get_mut(&mut self.last_meter).unwrap() = meter;
                    self.last_meter = METER.swap(self.last_meter.clone());
                    *Arc::get_mut(&mut self.last_meter).unwrap() = meter;
                }
            }
        }

        self.audio_graph.process(buf);

        if METER.load().playing {
            let last_meter = *self.last_meter;
            let mut meter = last_meter;
            meter.sample += buf.len();
            self.sender.try_send(last_meter.diff(meter)).unwrap();

            *Arc::get_mut(&mut self.last_meter).unwrap() = meter;
            self.last_meter = METER.swap(self.last_meter.clone());
            *Arc::get_mut(&mut self.last_meter).unwrap() = meter;
        }
    }
}
