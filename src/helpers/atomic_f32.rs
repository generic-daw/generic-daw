use std::sync::atomic::{AtomicU32, Ordering};

pub struct AtomicF32 {
    storage: AtomicU32,
}

impl AtomicF32 {
    pub fn new(value: f32) -> Self {
        Self {
            storage: AtomicU32::new(value.to_bits()),
        }
    }

    pub fn store(&self, value: f32, ordering: Ordering) {
        self.storage.store(value.to_bits(), ordering);
    }

    pub fn load(&self, ordering: Ordering) -> f32 {
        f32::from_bits(self.storage.load(ordering))
    }
}
