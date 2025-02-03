use audio_graph::{AudioGraph, AudioGraphNode};
use rtrb::{Consumer, Producer, RingBuffer};

mod audio_ctx_message;

pub use audio_ctx_message::AudioCtxMessage;

pub struct AudioCtx {
    audio_graph: AudioGraph,
    consumer: Consumer<AudioCtxMessage>,
}

impl AudioCtx {
    pub fn create(audio_graph: AudioGraphNode) -> (Self, Producer<AudioCtxMessage>) {
        let (producer, consumer) = RingBuffer::new(16);

        (
            Self {
                audio_graph: AudioGraph::new(audio_graph),
                consumer,
            },
            producer,
        )
    }

    #[expect(tail_expr_drop_order)]
    pub fn fill_buf(&mut self, buf_start_sample: usize, buf: &mut [f32]) {
        while let Ok(msg) = self.consumer.pop() {
            match msg {
                AudioCtxMessage::Add(node) => {
                    let ok = self.audio_graph.add(node);
                    debug_assert!(ok);
                }
                AudioCtxMessage::Remove(node) => {
                    let ok = self.audio_graph.remove(&node);
                    debug_assert!(ok);
                }
                AudioCtxMessage::Connect(from, to) => {
                    let ok = self.audio_graph.connect(&from, to);
                    debug_assert!(ok);
                }
                AudioCtxMessage::Disconnect(from, to) => {
                    let ok = self.audio_graph.disconnect(&from, &to);
                    debug_assert!(ok);
                }
            }
        }

        self.audio_graph.fill_buf(buf_start_sample, buf);
    }
}
