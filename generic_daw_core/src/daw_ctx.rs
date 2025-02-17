use crate::{master::Master, Meter};
use audio_graph::AudioGraph;
use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::Arc;

mod daw_ctx_message;

pub use daw_ctx_message::DawCtxMessage;

pub struct DawCtx<T> {
    pub meter: Arc<Meter>,
    audio_graph: AudioGraph,
    consumer: Consumer<DawCtxMessage<T>>,
}

impl<T> DawCtx<T> {
    pub(crate) fn create(sample_rate: u32) -> (Self, Producer<DawCtxMessage<T>>) {
        let (ui_producer, consumer) = RingBuffer::new(16);

        let meter = Arc::new(Meter::new(sample_rate));
        let master = Master::new(meter.clone());

        let audio_ctx = Self {
            audio_graph: AudioGraph::new(master.into()),
            consumer,
            meter,
        };

        (audio_ctx, ui_producer)
    }

    #[expect(tail_expr_drop_order)]
    pub fn fill_buf(&mut self, buf_start_sample: usize, buf: &mut [f32]) {
        while let Ok(msg) = self.consumer.pop() {
            match msg {
                DawCtxMessage::Insert(node) => {
                    self.audio_graph.insert(node);
                }
                DawCtxMessage::Remove(node) => {
                    self.audio_graph.remove(node);
                }
                DawCtxMessage::Connect(from, to) => {
                    self.audio_graph.connect(from, to);
                }
                DawCtxMessage::ConnectToMaster(node) => {
                    self.audio_graph.connect(self.audio_graph.root(), node);
                }
                DawCtxMessage::Disconnect(from, to) => {
                    self.audio_graph.disconnect(from, to);
                }
                DawCtxMessage::DisconnectFromMaster(node) => {
                    self.audio_graph.disconnect(self.audio_graph.root(), node);
                }
                DawCtxMessage::RequestAudioGraph(sender, t) => {
                    let audio_graph = std::mem::take(&mut self.audio_graph);
                    sender.send((audio_graph, t)).unwrap();
                }
                DawCtxMessage::AudioGraph(audio_graph) => {
                    self.audio_graph = audio_graph;
                }
            }
        }

        self.audio_graph.fill_buf(buf_start_sample, buf);
    }
}
