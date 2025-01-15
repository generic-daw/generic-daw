use crate::{pan, AudioGraphNodeImpl};
use atomig::Atomic;
use std::sync::{
    atomic::{AtomicUsize, Ordering::SeqCst},
    Mutex,
};

#[derive(Debug)]
pub struct MixerNode {
    /// 0 <= volume
    pub volume: Atomic<f32>,
    /// -1 <= pan <= 1
    pub pan: Atomic<f32>,
    buf: Mutex<Vec<f32>>,
    last_sample: AtomicUsize,
}

impl AudioGraphNodeImpl for MixerNode {
    fn fill_buf(&self, buf_start_sample: usize, buf: &mut [f32]) {
        let mut node_buf = self.buf.lock().unwrap();

        // we can assume the buffer size doesn't vary for the same buf_start_sample
        if buf_start_sample != self.last_sample.swap(buf_start_sample, SeqCst) {
            let volume = self.volume.load(SeqCst);
            let (mut lpan, mut rpan) = pan(self.pan.load(SeqCst));
            lpan *= volume;
            rpan *= volume;

            buf.iter_mut()
                .enumerate()
                .for_each(|(i, s)| *s *= if i % 2 == 0 { lpan } else { rpan });

            node_buf.clear();
            node_buf.extend(buf.iter().copied());
        }

        node_buf
            .iter()
            .zip(buf)
            .for_each(|(sample, buf)| *buf += sample);
    }
}

impl Default for MixerNode {
    fn default() -> Self {
        Self {
            volume: Atomic::new(1.0),
            pan: Atomic::default(),
            buf: Mutex::default(),
            last_sample: AtomicUsize::new(usize::MAX),
        }
    }
}
