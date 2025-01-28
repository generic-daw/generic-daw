use std::ops::AddAssign;

#[derive(Clone, Copy, Debug, Default)]
pub struct ArrangementPosition {
    /// position of the left of the arrangement relative to the start of the arrangement, in samples
    pub x: f32,
    /// position of the top of the arrangement relative to the top of the first track, in tracks
    pub y: f32,
}

impl ArrangementPosition {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

impl AddAssign for ArrangementPosition {
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}
