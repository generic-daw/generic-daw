use arc_swap::ArcSwap;
use atomig::Atomic;
use audio_graph::{AudioGraphNodeImpl, NodeId};
use clap_host::AudioProcessor;
use generic_daw_utils::ShiftMoveExt as _;
use std::{
    cmp::max_by,
    f32::consts::{FRAC_PI_4, SQRT_2},
    ops::Deref as _,
    sync::{
        Arc, Mutex,
        atomic::{
            AtomicBool,
            Ordering::{AcqRel, Acquire, Release},
        },
    },
};

#[derive(Debug)]
struct EffectEntry {
    effect: Mutex<AudioProcessor>,
    mix: Atomic<f32>,
    enabled: AtomicBool,
}

#[derive(Debug)]
pub struct MixerNode {
    id: NodeId,
    /// any effects that are to be applied to the input audio, before applying volume and pan
    effects: ArcSwap<Vec<Arc<EffectEntry>>>,
    /// in the `0.0..` range
    pub volume: Atomic<f32>,
    /// in the `-1.0..1.0` range
    pub pan: Atomic<f32>,
    /// whether the node is enabled
    pub enabled: AtomicBool,
    /// the maximum played back sample in the left channel
    max_l: Atomic<f32>,
    /// the maximum played back sample in the right channel
    max_r: Atomic<f32>,
}

impl MixerNode {
    pub fn get_l_r(&self) -> [f32; 2] {
        [self.max_l.swap(0.0, AcqRel), self.max_r.swap(0.0, AcqRel)]
    }

    pub fn add_effect(&self, effect: AudioProcessor) {
        self.with_effects_list(move |effects| {
            effects.push(Arc::new(EffectEntry {
                effect: Mutex::new(effect),
                mix: Atomic::new(1.0),
                enabled: AtomicBool::new(true),
            }));
        });
    }

    pub fn remove_effect(&self, index: usize) {
        self.with_effects_list(move |effects| {
            effects.remove(index);
        });
    }

    pub fn shift_move(&self, from: usize, to: usize) {
        self.with_effects_list(|effects| effects.shift_move(from, to));
    }

    #[must_use]
    pub fn get_effect_mix(&self, index: usize) -> f32 {
        self.effects.load()[index].mix.load(Acquire)
    }

    pub fn set_effect_mix(&self, index: usize, mix: f32) {
        self.effects.load()[index].mix.store(mix, Release);
    }

    #[must_use]
    pub fn get_effect_enabled(&self, index: usize) -> bool {
        self.effects.load()[index].enabled.load(Acquire)
    }

    pub fn set_effect_enabled(&self, index: usize, enabled: bool) {
        self.effects.load()[index].enabled.store(enabled, Release);
    }

    pub fn toggle_effect_enabled(&self, index: usize) -> bool {
        self.effects.load()[index].enabled.fetch_not(AcqRel)
    }

    fn with_effects_list(&self, f: impl FnOnce(&mut Vec<Arc<EffectEntry>>)) {
        let mut inner = self.effects.load().deref().deref().clone();
        f(&mut inner);
        self.effects.store(Arc::new(inner));
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

        self.effects
            .load()
            .iter()
            .filter(|entry| entry.enabled.load(Acquire))
            .for_each(|entry| {
                entry
                    .effect
                    .try_lock()
                    .expect("this is only locked from the audio thread")
                    .process(buf, entry.mix.load(Acquire));
            });

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

    fn id(&self) -> NodeId {
        self.id
    }

    fn reset(&self) {
        self.effects.load().iter().for_each(|entry| {
            entry
                .effect
                .try_lock()
                .expect("this is only locked from the audio thread")
                .reset();
        });
    }

    fn delay(&self) -> usize {
        self.effects
            .load()
            .iter()
            .map(|entry| {
                entry
                    .effect
                    .try_lock()
                    .expect("this is only locked from the audio thread")
                    .delay()
            })
            .sum()
    }
}

fn pan(angle: f32) -> [f32; 2] {
    let angle = (angle + 1.0) * FRAC_PI_4;

    [angle.cos(), angle.sin()].map(|s| s * SQRT_2)
}
