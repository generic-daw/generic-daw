use atomig::Atomic;
use clap_host::AudioProcessor;
use std::sync::{Mutex, atomic::AtomicBool};

#[derive(Debug)]
pub struct EffectEntry {
    pub effect: Mutex<AudioProcessor>,
    pub mix: Atomic<f32>,
    pub enabled: AtomicBool,
}
