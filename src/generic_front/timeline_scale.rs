use std::cell::Cell;

#[derive(Debug)]
pub struct TimelineScale {
    /// log2 of the horizontal scale
    pub x: Cell<f32>,
    /// height in pixels of each track in the timeline
    pub y: Cell<f32>,
}

impl Default for TimelineScale {
    fn default() -> Self {
        Self {
            x: Cell::new(8.0),
            y: Cell::new(100.0),
        }
    }
}
