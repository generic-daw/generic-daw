use crate::Position;
use std::ops::Add;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MidiNote {
    pub channel: u8,
    pub note: u16,
    /// between 0.0 and 1.0
    pub velocity: f64,
    /// start time of the note, relative to the beginning of the `MidiPattern` it belongs to
    pub start: Position,
    /// end time of the note, relative to the beginning of the `MidiPattern` it belongs to
    pub end: Position,
}

impl MidiNote {
    #[must_use]
    pub fn clamp(mut self, min: Position, max: Position) -> Option<Self> {
        if self.start > max || self.end < min {
            return None;
        }

        self.start = self.start.max(min);
        self.end = self.end.min(max);

        Some(self)
    }
}

impl Add<Position> for MidiNote {
    type Output = Self;

    fn add(mut self, rhs: Position) -> Self::Output {
        self.start += rhs;
        self.end += rhs;

        self
    }
}
