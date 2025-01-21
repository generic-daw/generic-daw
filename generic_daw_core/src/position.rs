use crate::{seconds_to_interleaved_samples, Meter};
use atomig::{Atom, AtomInteger};
use std::{
    fmt::{Debug, Formatter},
    ops::{Add, AddAssign, Sub, SubAssign},
    sync::atomic::Ordering::SeqCst,
};

#[derive(Atom, AtomInteger, Clone, Copy, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct Position(u32);

impl Debug for Position {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Position")
            .field("quarter_note", &self.quarter_note())
            .field("sub_quarter_note", &self.sub_quarter_note())
            .finish()
    }
}

impl Position {
    pub const QUARTER_NOTE: Self = Self::new(1, 0);
    pub const SUB_QUARTER_NOTE: Self = Self::new(0, 1);

    #[must_use]
    pub const fn new(quarter_note: u32, sub_quarter_note: u32) -> Self {
        debug_assert!(sub_quarter_note < 256);
        debug_assert!(quarter_note <= u32::MAX >> 8);

        Self(quarter_note << 8 | sub_quarter_note)
    }

    #[must_use]
    pub const fn quarter_note(self) -> u32 {
        self.0 >> 8
    }

    #[must_use]
    pub const fn sub_quarter_note(self) -> u32 {
        self.0 & 0xff
    }

    #[must_use]
    pub const fn floor(mut self) -> Self {
        self.0 &= !0xff;
        self
    }

    #[must_use]
    pub const fn ceil(mut self) -> Self {
        if self.sub_quarter_note() != 0 {
            self.0 &= !0xff;
            self.0 += 1 << 8;
        }

        self
    }

    #[must_use]
    pub fn from_interleaved_samples(samples: usize, meter: &Meter) -> Self {
        let global_beat = samples as f32 * f32::from(meter.bpm.load(SeqCst))
            / ((meter.sample_rate.load(SeqCst) * 120) as f32);
        Self::new(global_beat as u32, (global_beat.fract() * 256.0) as u32)
    }

    #[must_use]
    pub fn in_interleaved_samples_f(self, meter: &Meter) -> f32 {
        seconds_to_interleaved_samples(
            self.0 as f32 / 256.0 * 60.0 / f32::from(meter.bpm.load(SeqCst)),
            meter,
        )
    }

    #[must_use]
    pub fn in_interleaved_samples(self, meter: &Meter) -> usize {
        self.in_interleaved_samples_f(meter) as usize
    }

    #[must_use]
    pub fn snap(mut self, scale: f32, meter: &Meter) -> Self {
        let modulo = if scale < 12.0 {
            1 << (scale as u8 - 3)
        } else {
            (meter.numerator.load(SeqCst) as u32) << 8
        };

        let diff = self.0 % modulo;

        if diff >= modulo / 2 {
            self.0 += modulo - diff;
        } else {
            self.0 -= diff;
        }

        self
    }

    #[must_use]
    pub fn saturating_sub(self, other: Self) -> Self {
        Self(self.0.saturating_sub(other.0))
    }

    #[must_use]
    pub fn abs_diff(self, other: Self) -> Self {
        Self(self.0.abs_diff(other.0))
    }
}

impl Add for Position {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl AddAssign for Position {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

impl Sub for Position {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl SubAssign for Position {
    fn sub_assign(&mut self, rhs: Self) {
        self.0 -= rhs.0;
    }
}
