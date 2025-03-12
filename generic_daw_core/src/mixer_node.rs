use arc_swap::ArcSwap;
use atomig::Atomic;
use audio_graph::{AudioGraphNodeImpl, NodeId};
use clap_host::AudioProcessor;
use std::{
    cmp::max_by,
    f32::consts::{FRAC_PI_4, SQRT_2},
    sync::{
        Arc, Mutex,
        atomic::{
            AtomicBool,
            Ordering::{Acquire, Release},
        },
    },
};

#[derive(Debug)]
pub struct MixerNode {
    id: NodeId,
    /// any effects that are to be applied to the input audio, before applying volume and pan
    pub effects: ArcSwap<Vec<(Mutex<AudioProcessor>, Atomic<f32>)>>,
    /// 0 <= volume
    pub volume: Atomic<f32>,
    /// -1 <= pan <= 1
    pub pan: Atomic<f32>,
    /// whether the node is enabled
    pub enabled: AtomicBool,
    /// the maximum played back sample in the left channel
    max_l: Atomic<f32>,
    /// the maximum played back sample in the right channel
    max_r: Atomic<f32>,
    /// whether `max_l` and `max_r` have been read from
    read: AtomicBool,
}

impl MixerNode {
    pub fn get_l_r(&self) -> [f32; 2] {
        let arr = [self.max_l.load(Acquire), self.max_r.load(Acquire)];
        self.read.store(true, Release);
        arr
    }

    pub fn add_effect(&self, effect: AudioProcessor) {
        let mut effects = Arc::into_inner(self.effects.swap(Arc::new(vec![]))).unwrap();
        effects.push((Mutex::new(effect), Atomic::new(1.0)));
        self.effects.store(Arc::new(effects));
    }
}

impl Default for MixerNode {
    fn default() -> Self {
        Self {
            effects: ArcSwap::default(),
            id: NodeId::unique(),
            volume: Atomic::new(1.0),
            pan: Atomic::default(),
            enabled: AtomicBool::new(true),
            max_l: Atomic::default(),
            max_r: Atomic::default(),
            read: AtomicBool::new(false),
        }
    }
}

impl AudioGraphNodeImpl for MixerNode {
    fn fill_buf(&self, buf: &mut [f32]) {
        if !self.enabled.load(Acquire) {
            buf.iter_mut().for_each(|s| *s = 0.0);

            if self.read.load(Acquire) {
                self.max_l.store(0.0, Release);
                self.max_r.store(0.0, Release);
            }

            return;
        }

        let volume = self.volume.load(Acquire);
        let [lpan, rpan] = pan(self.pan.load(Acquire)).map(|s| s * volume);

        buf.iter_mut()
            .enumerate()
            .for_each(|(i, s)| *s *= if i % 2 == 0 { lpan } else { rpan });

        for (effect, mix) in &**self.effects.load() {
            effect
                .try_lock()
                .expect("this is only locked from the audio thread")
                .process(buf, mix.load(Acquire));
        }

        let cur_l = buf
            .iter()
            .step_by(2)
            .copied()
            .map(f32::abs)
            .max_by(f32::total_cmp)
            .unwrap();
        let cur_r = buf
            .iter()
            .skip(1)
            .step_by(2)
            .copied()
            .map(f32::abs)
            .max_by(f32::total_cmp)
            .unwrap();

        if self.read.load(Acquire) {
            self.max_l.store(cur_l, Release);
            self.max_r.store(cur_r, Release);
        } else {
            self.max_l
                .fetch_update(Release, Acquire, |max_l| {
                    Some(max_by(max_l, cur_l, f32::total_cmp))
                })
                .unwrap();
            self.max_r
                .fetch_update(Release, Acquire, |max_r| {
                    Some(max_by(max_r, cur_r, f32::total_cmp))
                })
                .unwrap();
        }
    }

    fn id(&self) -> NodeId {
        self.id
    }
}

fn pan(angle: f32) -> [f32; 2] {
    let angle = (angle + 1.0) * FRAC_PI_4;

    [angle.cos(), angle.sin()].map(|s| s * SQRT_2)
}
