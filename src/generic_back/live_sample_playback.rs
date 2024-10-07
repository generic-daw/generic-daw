use std::sync::{
    atomic::{AtomicUsize, Ordering::SeqCst},
    Arc,
};

#[derive(Debug)]
pub struct PlayingBack {
    sample: Arc<Vec<f32>>,
    current_sample: AtomicUsize,
}

impl PlayingBack {
    pub fn new(sample: Arc<Vec<f32>>) -> Self {
        Self {
            sample,
            current_sample: AtomicUsize::default(),
        }
    }

    pub fn get(&self) -> f32 {
        self.sample[self.current_sample.fetch_add(1, SeqCst)]
    }

    pub fn over(&self) -> bool {
        self.current_sample.load(SeqCst) >= self.sample.len()
    }
}
