#[derive(Clone, Copy, Debug)]
pub struct TimelineScale {
    /// log2 of the horizontal scale
    pub x: f32,
    /// height in pixels of each track in the timeline
    pub y: f32,
}

impl Default for TimelineScale {
    fn default() -> Self {
        Self { x: 8.0, y: 100.0 }
    }
}
