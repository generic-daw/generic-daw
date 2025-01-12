use atomig::Atom;

use crate::generic_back::{seconds_to_interleaved_samples, Meter};
use std::{
    cmp::Ordering,
    ops::{Add, AddAssign, Sub, SubAssign},
    sync::atomic::Ordering::SeqCst,
};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Position {
    /// the position in quarter notes, rounded down
    pub quarter_note: u16,
    /// the position relative to `quarter_note`, in 256ths of a quarter note
    pub sub_quarter_note: u8,
    _padding: u8,
}

impl Atom for Position {
    type Repr = u32;

    fn pack(self) -> Self::Repr {
        // SAFETY:
        // any `Position` is a valid u32
        unsafe { std::mem::transmute(self) }
    }

    fn unpack(src: Self::Repr) -> Self {
        // SAFETY:
        // any u32 is a valid `Position`
        unsafe { std::mem::transmute(src) }
    }
}

impl Position {
    pub const MAX: Self = Self {
        quarter_note: u16::MAX,
        sub_quarter_note: u8::MAX,
        _padding: 0,
    };

    pub const MIN_STEP: Self = Self {
        quarter_note: 0,
        sub_quarter_note: 1,
        _padding: 0,
    };

    pub fn new(quarter_note: u16, sub_quarter_note: u8) -> Self {
        Self {
            quarter_note,
            sub_quarter_note,
            _padding: 0,
        }
    }

    pub fn from_interleaved_samples(samples: usize, meter: &Meter) -> Self {
        let global_beat = samples as f32 * f32::from(meter.bpm.load(SeqCst))
            / (meter.sample_rate.load(SeqCst) as f32 * 120.0);
        let quarter_note = global_beat as u16;
        let sub_quarter_note = ((global_beat - f32::from(quarter_note)) * 256.0) as u8;

        Self::new(quarter_note, sub_quarter_note)
    }

    pub fn in_interleaved_samples_f(self, meter: &Meter) -> f32 {
        let global_beat = f32::from(self.quarter_note) + f32::from(self.sub_quarter_note) / 256.0;

        seconds_to_interleaved_samples(
            global_beat * 60.0 / f32::from(meter.bpm.load(SeqCst)),
            meter,
        )
    }

    pub fn in_interleaved_samples(self, meter: &Meter) -> usize {
        self.in_interleaved_samples_f(meter) as usize
    }

    pub fn snap(mut self, scale: f32, meter: &Meter) -> Self {
        if scale < 11.0 {
            let shift = 1u8 << (scale as u8 - 3);
            let lower = self.sub_quarter_note - self.sub_quarter_note % shift;
            let upper = lower.checked_add(shift);

            if self.sub_quarter_note - lower < upper.unwrap_or(255) - self.sub_quarter_note {
                self.sub_quarter_note = lower;
            } else if let Some(upper) = upper {
                self.sub_quarter_note = upper;
            } else {
                self.sub_quarter_note = 0;
                self.quarter_note += 1;
            }
        } else if scale < 12.0 {
            if self.sub_quarter_note >= 128 {
                self.quarter_note += 1;
            }
            self.sub_quarter_note = 0;
        } else {
            self.sub_quarter_note = 0;

            let shift = meter.numerator.load(SeqCst) as u16;
            let lower = self.quarter_note - self.quarter_note % shift;
            let upper = lower + shift;

            if self.quarter_note - lower < upper - self.quarter_note {
                self.quarter_note = lower;
            } else {
                self.quarter_note = upper;
            }
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

    pub fn abs_diff(self, other: Self) -> Self {
        if self < other {
            other - self
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
        debug_assert!(Self::MAX - self >= rhs);

        Self::new(
            self.quarter_note
                + rhs.quarter_note
                + u16::from(self.sub_quarter_note > u8::MAX - rhs.sub_quarter_note),
            self.sub_quarter_note.wrapping_add(rhs.sub_quarter_note),
        )
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
        debug_assert!(self >= rhs);

        Self::new(
            self.quarter_note
                - rhs.quarter_note
                - u16::from(self.sub_quarter_note < rhs.sub_quarter_note),
            self.sub_quarter_note.wrapping_sub(rhs.sub_quarter_note),
        )
    }
}

impl SubAssign for Position {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}
