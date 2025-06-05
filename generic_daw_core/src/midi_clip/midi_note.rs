use super::MidiKey;
use crate::MusicalTime;
use std::ops::Add;

#[derive(Clone, Copy, Debug)]
pub struct MidiNote {
    /// usually in the `0..15` range
    pub channel: u8,
    /// in the `0..=127` range
    pub key: MidiKey,
    /// in the `0.0..=1.0` range
    pub velocity: f64,
    /// start time of the note, relative to the beginning of the pattern it belongs to
    pub start: MusicalTime,
    /// end time of the note, relative to the beginning of the pattern it belongs to
    pub end: MusicalTime,
}

impl MidiNote {
    #[must_use]
    pub fn clamp(mut self, min: MusicalTime, max: MusicalTime) -> Option<Self> {
        if self.start > max || self.end < min {
            return None;
        }

        self.start = self.start.max(min);
        self.end = self.end.min(max);

        Some(self)
    }

    #[must_use]
    pub fn saturating_sub(mut self, other: MusicalTime) -> Option<Self> {
        if self.end < other {
            return None;
        }

        self.start = self.start.saturating_sub(other);
        self.end -= other;

        Some(self)
    }

    pub fn trim_start_to(&mut self, new_start: MusicalTime) {
        self.start = new_start.min(self.end - MusicalTime::TICK);
    }

    pub fn trim_end_to(&mut self, new_end: MusicalTime) {
        self.end = new_end.max(self.start + MusicalTime::TICK);
    }

    pub fn move_to(&mut self, new_start: MusicalTime) {
        let diff = self.start.abs_diff(new_start);

        if self.start < new_start {
            self.end += diff;
        } else {
            self.end -= diff;
        }

        self.start = new_start;
    }
}

impl Add<MusicalTime> for MidiNote {
    type Output = Self;

    fn add(mut self, rhs: MusicalTime) -> Self::Output {
        self.start += rhs;
        self.end += rhs;

        self
    }
}
