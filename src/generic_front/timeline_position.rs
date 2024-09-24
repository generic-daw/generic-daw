#[derive(Clone, Copy, Debug, Default)]
pub struct TimelinePosition {
    /// position of the left of the timeline relative to the start of the arrangement, in samples
    pub x: f32,
    /// position of the top of the timeline relative to the top of the first track, in tracks
    pub y: f32,
}
