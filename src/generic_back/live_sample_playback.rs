use crate::generic_back::InterleavedAudio;
use std::sync::{
    atomic::{AtomicUsize, Ordering::SeqCst},
    Arc,
};

#[derive(Debug)]
pub struct PlayingBack {
    sample: Arc<InterleavedAudio>,
    current_sample: AtomicUsize,
}

impl PlayingBack {
    pub fn new(sample: Arc<InterleavedAudio>) -> Self {
        Self {
            sample,
            current_sample: AtomicUsize::default(),
        }
    }

    pub fn get(&self) -> f32 {
        let current_sample = self.current_sample.fetch_add(1, SeqCst);
        if current_sample >= self.sample.samples.len() {
            0.0
        } else {
            self.sample.samples[current_sample]
        }
    }
}
