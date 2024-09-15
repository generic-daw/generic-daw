use std::sync::RwLock;

pub struct TimelineScale {
    /// log2 of the horizontal scale
    pub x: f32,
    /// height in pixels of each track in the timeline
    pub y: f32,
}

impl TimelineScale {
    pub const fn create() -> RwLock<Self> {
        RwLock::new(Self { x: 8.0, y: 100.0 })
    }
}
