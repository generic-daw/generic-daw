use crate::Position;
use generic_daw_utils::unique_id;
pub use note_id::Id as NoteId;
use std::ops::Add;

unique_id!(note_id);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MidiNote {
    /// usually in the `0..15` range
    pub channel: u8,
    /// `60` is Middle C, in the `0..=127` range
    pub key: u8,
    /// uniquely identify this note to the plugin, in the `0..i32::MAX` range
    pub note_id: NoteId,
    /// in the `0.0..1.0` range
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

    #[must_use]
    pub fn saturating_sub(mut self, other: Position) -> Option<Self> {
        if self.end < other {
            return None;
        }

        self.start = self.start.saturating_sub(other);
        self.end -= other;

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
