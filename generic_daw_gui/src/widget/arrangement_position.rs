use std::cell::Cell;

#[derive(Debug, Default)]
pub struct ArrangementPosition {
    /// position of the left of the arrangement relative to the start of the arrangement, in samples
    pub x: Cell<f32>,
    /// position of the top of the arrangement relative to the top of the first track, in tracks
    pub y: Cell<f32>,
}
