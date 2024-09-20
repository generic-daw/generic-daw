use crate::helpers::AtomicF32;

pub struct TimelinePosition {
    /// position of the left of the timeline relative to the start of the arrangement, in samples
    pub x: AtomicF32,
    /// position of the top of the timeline relative to the top of the first track, in tracks
    pub y: AtomicF32,
}

impl Default for TimelinePosition {
    fn default() -> Self {
        Self::new()
    }
}

impl TimelinePosition {
    pub fn new() -> Self {
        Self {
            x: AtomicF32::new(0.0),
            y: AtomicF32::new(0.0),
        }
    }
}
