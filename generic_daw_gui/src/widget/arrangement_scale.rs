use super::LINE_HEIGHT;
use std::ops::AddAssign;

#[derive(Clone, Copy, Debug, PartialEq)]
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
    pub const ZERO: Self = Self::new(0.0, 0.0);

    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn clamp(mut self) -> Self {
        self.x = self.x.clamp(3.0, 12.999_999);
        self.y = self.y.clamp(2.0 * LINE_HEIGHT, 10.0 * LINE_HEIGHT);
        self
    }
}

impl Default for ArrangementScale {
    fn default() -> Self {
        Self { x: 9.0, y: 120.0 }
    }
}

impl AddAssign for ArrangementScale {
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}
