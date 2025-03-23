use super::MidiKey;
use crate::Position;
use generic_daw_utils::unique_id;
pub use note_id::Id as NoteId;
use std::ops::Add;

unique_id!(note_id, u32);

#[derive(Clone, Copy, Debug)]
pub struct MidiNote {
    /// usually in the `0..15` range
    pub channel: u8,
    /// key of the note
    pub key: MidiKey,
    /// uniquely identify this note to the plugin, in the `0..i32::MAX` range
    pub note_id: NoteId,
    /// in the `0.0..1.0` range
    pub velocity: f64,
    /// start time of the note, relative to the beginning of the pattern it belongs to
    pub start: Position,
    /// end time of the note, relative to the beginning of the pattern it belongs to
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

    pub fn trim_start_to(&mut self, new_global_start: Position) {
        self.start = new_global_start.min(self.end - Position::STEP);
    }

    pub fn trim_end_to(&mut self, new_global_end: Position) {
        self.end = new_global_end.max(self.start + Position::STEP);
    }

    pub fn move_to(&mut self, new_global_start: Position) {
        let diff = self.start.abs_diff(new_global_start);

        if self.start < new_global_start {
            self.end += diff;
        } else {
            self.end -= diff;
        }

        self.start = new_global_start;
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
