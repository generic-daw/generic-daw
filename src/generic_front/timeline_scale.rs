use crate::helpers::atomic_f32::AtomicF32;

pub struct TimelineScale {
    /// log2 of the horizontal scale
    pub x: AtomicF32,
    /// height in pixels of each track in the timeline
    pub y: AtomicF32,
}

impl Default for TimelineScale {
    fn default() -> Self {
        Self::new()
    }
}

impl TimelineScale {
    pub fn new() -> Self {
        Self {
            x: AtomicF32::new(8.0),
            y: AtomicF32::new(100.0),
        }
    }
}
