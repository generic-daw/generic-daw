use crate::generic_back::{seconds_to_interleaved_samples, Meter};
use std::{
    cmp::Ordering,
    ops::{Add, AddAssign, Sub, SubAssign},
    sync::{atomic::Ordering::SeqCst, Arc},
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Position {
    /// the position in quarter notes, rounded down
    pub quarter_note: u16,
    /// the position relative to `quarter_note`, in 256ths of a quarter note
    pub sub_quarter_note: u8,
}

pub const POSITION_MIN_STEP: Position = Position {
    quarter_note: 0,
    sub_quarter_note: 1,
};

pub const POSITION_MAX: Position = Position {
    quarter_note: 65535,
    sub_quarter_note: 255,
};

impl Position {
    pub fn new(quarter_note: u16, sub_quarter_note: u8) -> Self {
        Self {
            quarter_note,
            sub_quarter_note,
        }
    }

    pub fn from_interleaved_samples(samples: u32, meter: &Arc<Meter>) -> Self {
        let global_beat = f64::from(samples)
            / (f64::from(meter.sample_rate.load(SeqCst) * 2)
                / (f64::from(meter.bpm.load(SeqCst)) / 60.0));
        let quarter_note = global_beat as u16;
        let sub_quarter_note = ((global_beat - f64::from(quarter_note)) * 256.0) as u8;

        Self {
            quarter_note,
            sub_quarter_note,
        }
    }

    pub fn in_interleaved_samples(self, meter: &Arc<Meter>) -> u32 {
        let global_beat = f64::from(self.quarter_note) + f64::from(self.sub_quarter_note) / 256.0;

        seconds_to_interleaved_samples(
            global_beat * 60.0 / f64::from(meter.bpm.load(SeqCst)),
            meter,
        )
    }

    pub fn snap(mut self, scale: f32) -> Self {
        self.sub_quarter_note -=
            self.sub_quarter_note % (1u8.checked_shl(scale as u32 - 3).unwrap_or(0));
        if scale > 11f32 {
            self.quarter_note -= self.quarter_note % 4;
        }
        self
    }

    pub fn saturating_sub(self, other: Self) -> Self {
        if self <= other {
            Self::default()
        } else {
            self - other
        }
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
        assert!(POSITION_MAX - self >= rhs);

        Self {
            quarter_note: self.quarter_note
                + rhs.quarter_note
                + u16::from(self.sub_quarter_note > u8::MAX - rhs.sub_quarter_note),
            sub_quarter_note: self.sub_quarter_note.wrapping_add(rhs.sub_quarter_note),
        }
    }
}

impl AddAssign for Position {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl Sub for Position {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        assert!(self >= rhs);

        Self {
            quarter_note: self.quarter_note
                - rhs.quarter_note
                - u16::from(self.sub_quarter_note < rhs.sub_quarter_note),
            sub_quarter_note: self.sub_quarter_note.wrapping_sub(rhs.sub_quarter_note),
        }
    }
}

impl SubAssign for Position {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}
