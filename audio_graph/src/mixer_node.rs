use crate::{pan, AudioGraphNodeImpl};
use atomig::Atomic;
use std::sync::atomic::Ordering::SeqCst;

#[derive(Debug)]
pub struct MixerNode {
    /// 0 <= volume
    pub volume: Atomic<f32>,
    /// -1 <= pan <= 1
    pub pan: Atomic<f32>,
}

impl Default for MixerNode {
    fn default() -> Self {
        Self {
            volume: Atomic::new(1.0),
            pan: Atomic::default(),
        }
    }
}

impl AudioGraphNodeImpl for MixerNode {
    fn fill_buf(&self, _buf_start_sample: usize, buf: &mut [f32]) {
        let volume = self.volume.load(SeqCst);
        let [lpan, rpan] = pan(self.pan.load(SeqCst)).map(|s| s * volume);

        buf.iter_mut()
            .enumerate()
            .for_each(|(i, s)| *s *= if i % 2 == 0 { lpan } else { rpan });
    }
}
