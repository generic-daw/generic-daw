use super::LINE_HEIGHT;
use std::cell::Cell;

#[derive(Debug)]
pub struct ArrangementScale {
    /// log2 of the horizontal scale
    ///
    /// 3.0 <= x < 13.0
    pub x: Cell<f32>,
    /// height in pixels of each track in the timeline
    ///
    /// 32.0 <= x <= 200.0
    pub y: Cell<f32>,
}

impl Default for ArrangementScale {
    fn default() -> Self {
        Self {
            x: Cell::new(8.0),
            y: Cell::new(LINE_HEIGHT * 5.0),
        }
    }
}
