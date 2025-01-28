use super::LINE_HEIGHT;
use std::ops::AddAssign;

#[derive(Clone, Copy, Debug)]
pub struct ArrangementScale {
    /// log2 of the number of audio samples per pixel
    ///
    /// 3.0 <= x < 13.0
    pub x: f32,
    /// height in pixels of each track in the arrangement
    ///
    /// 42.0 <= x <= 210.0
    pub y: f32,
}

impl ArrangementScale {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn clamp(mut self, min_x: f32, max_x: f32, min_y: f32, max_y: f32) -> Self {
        self.x = self.x.clamp(min_x, max_x);
        self.y = self.y.clamp(min_y, max_y);
        self
    }
}

impl Default for ArrangementScale {
    fn default() -> Self {
        Self {
            x: 8.0,
            y: LINE_HEIGHT * 5.0,
        }
    }
}

impl AddAssign for ArrangementScale {
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}
