use crate::Meter;
use audio_graph::{AudioGraph, AudioGraphNode};
use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::Arc;

mod audio_ctx_message;
mod ui_message;

pub use audio_ctx_message::AudioCtxMessage;
pub use ui_message::UiMessage;

pub struct AudioCtx<T> {
    pub meter: Arc<Meter>,
    audio_graph: AudioGraph,
    producer: Producer<UiMessage<T>>,
    consumer: Consumer<AudioCtxMessage<T>>,
}

impl<T> AudioCtx<T> {
    pub(crate) fn create(
        audio_graph: AudioGraphNode,
        meter: Arc<Meter>,
    ) -> (Self, Producer<AudioCtxMessage<T>>, Consumer<UiMessage<T>>) {
        let (ui_producer, consumer) = RingBuffer::new(16);
        let (producer, ui_consumer) = RingBuffer::new(16);

        let audio_ctx = Self {
            audio_graph: AudioGraph::new(audio_graph),
            producer,
            consumer,
            meter,
        };

        (audio_ctx, ui_producer, ui_consumer)
    }

    #[expect(tail_expr_drop_order)]
    pub fn fill_buf(&mut self, buf_start_sample: usize, buf: &mut [f32]) {
        while let Ok(msg) = self.consumer.pop() {
            match msg {
                AudioCtxMessage::Insert(node) => {
                    self.audio_graph.insert(node);
                }
                AudioCtxMessage::Remove(node) => {
                    self.audio_graph.remove(node);
                }
                AudioCtxMessage::Connect(from, to) => {
                    self.audio_graph.connect(from, to);
                }
                AudioCtxMessage::ConnectToMaster(node) => {
                    self.audio_graph.connect(self.audio_graph.root(), node);
                }
                AudioCtxMessage::Disconnect(from, to) => {
                    self.audio_graph.disconnect(from, to);
                }
                AudioCtxMessage::DisconnectFromMaster(node) => {
                    self.audio_graph.disconnect(self.audio_graph.root(), node);
                }
                AudioCtxMessage::RequestAudioGraph(a) => {
                    let audio_graph = std::mem::take(&mut self.audio_graph);
                    self.producer
                        .push(UiMessage::AudioGraph(a, audio_graph))
                        .unwrap();
                }
                AudioCtxMessage::AudioGraph(audio_graph) => {
                    self.audio_graph = audio_graph;
                }
            }
        }

        self.audio_graph.fill_buf(buf_start_sample, buf);
    }
}
