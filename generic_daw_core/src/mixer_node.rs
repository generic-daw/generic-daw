use crate::event::Event;
use arc_swap::ArcSwap;
use atomig::Atomic;
use audio_graph::{NodeId, NodeImpl};
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
struct Plugin {
    processor: Mutex<AudioProcessor>,
    mix: Atomic<f32>,
    enabled: AtomicBool,
}

#[derive(Debug)]
pub struct MixerNode {
    id: NodeId,
    /// any plugins that are to be applied to the input audio, before applying volume and pan
    plugins: ArcSwap<Vec<Arc<Plugin>>>,
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

impl NodeImpl<Event> for MixerNode {
    fn process(&self, audio: &mut [f32], events: &mut Vec<Event>) {
        if !self.enabled.load(Acquire) {
            audio.iter_mut().for_each(|s| *s = 0.0);
            events.clear();

            return;
        }

        self.plugins
            .load()
            .iter()
            .filter(|entry| entry.enabled.load(Acquire))
            .for_each(|entry| {
                entry
                    .processor
                    .try_lock()
                    .expect("this is only locked from the audio thread")
                    .process(audio, events, entry.mix.load(Acquire));
            });

        let volume = self.volume.load(Acquire);
        let [lpan, rpan] = pan(self.pan.load(Acquire)).map(|s| s * volume);

        audio
            .iter_mut()
            .enumerate()
            .for_each(|(i, s)| *s *= if i % 2 == 0 { lpan } else { rpan });

        let cur_l = audio
            .iter()
            .step_by(2)
            .copied()
            .map(f32::abs)
            .max_by(f32::total_cmp)
            .unwrap();
        let cur_r = audio
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
        self.plugins.load().iter().for_each(|entry| {
            entry
                .processor
                .try_lock()
                .expect("this is only locked from the audio thread")
                .reset();
        });
    }

    fn delay(&self) -> usize {
        self.plugins
            .load()
            .iter()
            .map(|entry| {
                entry
                    .processor
                    .try_lock()
                    .expect("this is only locked from the audio thread")
                    .delay()
            })
            .sum()
    }
}

impl MixerNode {
    pub fn get_l_r(&self) -> [f32; 2] {
        [self.max_l.swap(0.0, AcqRel), self.max_r.swap(0.0, AcqRel)]
    }

    pub fn add_plugin(&self, processor: AudioProcessor) {
        self.with_plugins_list(move |plugins| {
            plugins.push(Arc::new(Plugin {
                processor: Mutex::new(processor),
                mix: Atomic::new(1.0),
                enabled: AtomicBool::new(true),
            }));
        });
    }

    pub fn remove_plugin(&self, index: usize) {
        self.with_plugins_list(move |plugins| {
            plugins.remove(index);
        });
    }

    pub fn shift_move(&self, from: usize, to: usize) {
        self.with_plugins_list(|plugins| plugins.shift_move(from, to));
    }

    #[must_use]
    pub fn get_plugin_mix(&self, index: usize) -> f32 {
        self.plugins.load()[index].mix.load(Acquire)
    }

    pub fn set_plugin_mix(&self, index: usize, mix: f32) {
        self.plugins.load()[index].mix.store(mix, Release);
    }

    #[must_use]
    pub fn get_plugin_enabled(&self, index: usize) -> bool {
        self.plugins.load()[index].enabled.load(Acquire)
    }

    pub fn set_plugin_enabled(&self, index: usize, enabled: bool) {
        self.plugins.load()[index].enabled.store(enabled, Release);
    }

    pub fn toggle_plugin_enabled(&self, index: usize) -> bool {
        self.plugins.load()[index].enabled.fetch_not(AcqRel)
    }

    fn with_plugins_list(&self, f: impl FnOnce(&mut Vec<Arc<Plugin>>)) {
        let mut inner = self.plugins.load().deref().deref().clone();
        f(&mut inner);
        self.plugins.store(Arc::new(inner));
    }
}

impl Default for MixerNode {
    fn default() -> Self {
        Self {
            plugins: ArcSwap::default(),
            id: NodeId::unique(),
            volume: Atomic::new(1.0),
            pan: Atomic::default(),
            enabled: AtomicBool::new(true),
            max_l: Atomic::default(),
            max_r: Atomic::default(),
        }
    }
}

fn pan(pan: f32) -> [f32; 2] {
    let angle = (pan + 1.0) * FRAC_PI_4;

    let (r, l) = angle.sin_cos();

    [l * SQRT_2, r * SQRT_2]
}
