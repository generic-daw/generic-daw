use crate::{AudioGraphNodeImpl, NodeId, pan};
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
    id: NodeId,
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
            id: NodeId::unique(),
            volume: Atomic::new(1.0),
            pan: Atomic::default(),
            enabled: AtomicBool::new(true),
            max_l: Atomic::default(),
            max_r: Atomic::default(),
        }
    }
}

impl AudioGraphNodeImpl for MixerNode {
    fn fill_buf(&self, buf: &mut [f32]) {
        if !self.enabled.load(Acquire) {
            buf.iter_mut().for_each(|s| *s = 0.0);
            return;
        }

        let volume = self.volume.load(Acquire);
        let [lpan, rpan] = pan(self.pan.load(Acquire)).map(|s| s * volume);

        buf.iter_mut()
            .enumerate()
            .for_each(|(i, s)| *s *= if i % 2 == 0 { lpan } else { rpan });

        let cur_l = buf
            .iter()
            .step_by(2)
            .copied()
            .map(f32::abs)
            .max_by(f32::total_cmp)
            .unwrap();

        self.max_l
            .fetch_update(Release, Acquire, |max_l| {
                Some(max_by(max_l, cur_l, f32::total_cmp))
            })
            .unwrap();

        let cur_r = buf
            .iter()
            .skip(1)
            .step_by(2)
            .copied()
            .map(f32::abs)
            .max_by(f32::total_cmp)
            .unwrap();

        self.max_r
            .fetch_update(Release, Acquire, |max_r| {
                Some(max_by(max_r, cur_r, f32::total_cmp))
            })
            .unwrap();
    }

    fn id(&self) -> NodeId {
        self.id
    }
}
