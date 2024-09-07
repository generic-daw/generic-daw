use crate::generic_back::meter::Meter;
use iced::{widget::canvas::Frame, Theme};

pub struct TimelinePosition {
    /// position of the left of the timeline relative to the start of the arrangement, in samples
    pub x: f32,
    /// position of the top of the timeline relative to the top of the first track, in tracks
    pub y: f32,
}

#[derive(Clone, Copy)]
pub struct TimelineScale {
    /// log2 of the horizontal scale
    pub x: f32,
    /// height in pixels of each track in the timeline
    pub y: f32,
}

pub trait Drawable {
    fn draw(
        &self,
        frame: &mut Frame,
        scale: TimelineScale,
        offset: &TimelinePosition,
        meter: &Meter,
        theme: &Theme,
    );
}
