use crate::generic_back::{seconds_to_interleaved_samples, Meter};
use std::{
    cmp::Ordering,
    ops::{Add, AddAssign, Sub, SubAssign},
    sync::atomic::Ordering::SeqCst,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Position {
    /// the position in quarter notes, rounded down
    pub quarter_note: u16,
    /// the position relative to `quarter_note`, in 256ths of a quarter note
    pub sub_quarter_note: u16,
}

impl Position {
    pub const fn new(quarter_note: u16, sub_quarter_note: u16) -> Self {
        Self {
            quarter_note,
            sub_quarter_note,
        }
    }

    pub fn from_interleaved_samples(samples: u32, meter: &Meter) -> Self {
        let global_beat = f64::from(samples)
            / (f64::from(meter.sample_rate.load(SeqCst) * 2)
                / (f64::from(meter.bpm.load(SeqCst)) / 60.0));
        let quarter_note = global_beat as u16;
        let sub_quarter_note = (global_beat - f64::from(quarter_note)) as u16 * 256;

        Self {
            quarter_note,
            sub_quarter_note,
        }
    }

    pub fn in_interleaved_samples(self, meter: &Meter) -> u32 {
        let global_beat = f64::from(self.quarter_note) + f64::from(self.sub_quarter_note) / 256.0;

        seconds_to_interleaved_samples(
            global_beat * 60.0 / f64::from(meter.bpm.load(SeqCst)),
            meter,
        )
    }
}

impl PartialOrd for Position {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Position {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.quarter_note.cmp(&other.quarter_note) {
            Ordering::Equal => self.sub_quarter_note.cmp(&other.sub_quarter_note),
            other => other,
        }
    }
}

impl Add for Position {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        let new_sub_quarter_note = self.sub_quarter_note + rhs.sub_quarter_note;
        Self {
            quarter_note: self.quarter_note + rhs.quarter_note + new_sub_quarter_note / 256,
            sub_quarter_note: new_sub_quarter_note % 256,
        }
    }
}

impl AddAssign for Position {
    fn add_assign(&mut self, rhs: Self) {
        let new = *self + rhs;
        self.quarter_note = new.quarter_note;
        self.sub_quarter_note = new.sub_quarter_note;
    }
}

impl Sub for Position {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        assert!(self >= rhs);

        if self.sub_quarter_note > rhs.sub_quarter_note {
            Self {
                quarter_note: self.quarter_note - rhs.quarter_note,
                sub_quarter_note: self.sub_quarter_note - rhs.sub_quarter_note,
            }
        } else {
            Self {
                quarter_note: self.quarter_note - rhs.quarter_note - 1,
                sub_quarter_note: 256 + self.sub_quarter_note - rhs.sub_quarter_note,
            }
        }
    }
}

impl SubAssign for Position {
    fn sub_assign(&mut self, rhs: Self) {
        let new = *self - rhs;
        self.quarter_note = new.quarter_note;
        self.sub_quarter_note = new.sub_quarter_note;
    }
}
