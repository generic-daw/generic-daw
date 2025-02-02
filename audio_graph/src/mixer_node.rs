use crate::{pan, AudioGraphNodeImpl};
use atomig::Atomic;
use std::{
    cmp::max_by,
    sync::atomic::{
        AtomicBool,
        Ordering::{Acquire, Release},
    },
};

#[derive(Debug)]
pub struct MixerNode {
    /// 0 <= volume
    pub volume: Atomic<f32>,
    /// -1 <= pan <= 1
    pub pan: Atomic<f32>,
    /// whether the node is enabled
    pub enabled: AtomicBool,
    /// the maximum played back sample in the left channel
    pub max_l: Atomic<f32>,
    /// the maximum played back sample in the right channel
    pub max_r: Atomic<f32>,
}

impl Default for MixerNode {
    fn default() -> Self {
        Self {
            volume: Atomic::new(1.0),
            pan: Atomic::default(),
            enabled: AtomicBool::new(true),
            max_l: Atomic::default(),
            max_r: Atomic::default(),
        }
    }
}

impl AudioGraphNodeImpl for MixerNode {
    fn fill_buf(&self, _buf_start_sample: usize, buf: &mut [f32]) {
        if !self.enabled.load(Acquire) {
            buf.iter_mut().for_each(|s| *s = 0.0);
            return;
        }

        let volume = self.volume.load(Acquire);
        let [lpan, rpan] = pan(self.pan.load(Acquire)).map(|s| s * volume);

        buf.iter_mut()
            .enumerate()
            .for_each(|(i, s)| *s *= if i % 2 == 0 { lpan } else { rpan });

        self.max_l.store(
            max_by(
                self.max_l.load(Acquire),
                buf.iter()
                    .step_by(2)
                    .copied()
                    .map(f32::abs)
                    .max_by(f32::total_cmp)
                    .unwrap(),
                f32::total_cmp,
            ),
            Release,
        );

        self.max_r.store(
            max_by(
                self.max_r.load(Acquire),
                buf.iter()
                    .skip(1)
                    .step_by(2)
                    .copied()
                    .map(f32::abs)
                    .max_by(f32::total_cmp)
                    .unwrap(),
                f32::total_cmp,
            ),
            Release,
        );
    }
}
