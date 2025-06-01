use crate::Meter;
use atomig::{Atom, AtomInteger};
use std::{
    fmt::{Debug, Formatter},
    ops::{Add, AddAssign, Mul, Sub, SubAssign},
};

#[derive(Atom, AtomInteger, Clone, Copy, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct Position(u32);

impl Debug for Position {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Position")
            .field("beat", &self.beat())
            .field("step", &self.step())
            .finish()
    }
}

impl Position {
    pub const ZERO: Self = Self::new(0, 0);
    pub const BEAT: Self = Self::new(1, 0);
    pub const STEP: Self = Self::new(0, 1);

    #[must_use]
    pub const fn new(quarter_note: u32, sub_quarter_note: u32) -> Self {
        debug_assert!(sub_quarter_note < 256);
        debug_assert!(quarter_note <= u32::MAX >> 8);

        Self((quarter_note << 8) | sub_quarter_note)
    }

    #[must_use]
    pub const fn beat(self) -> u32 {
        self.0 >> 8
    }

    #[must_use]
    pub const fn step(self) -> u32 {
        self.0 & 0xff
    }

    #[must_use]
    pub const fn floor(mut self) -> Self {
        self.0 &= !0xff;
        self
    }

    #[must_use]
    pub const fn ceil(mut self) -> Self {
        if self.step() != 0 {
            self.0 &= !0xff;
            self.0 += 1 << 8;
        }

        self
    }

    #[must_use]
    pub const fn round(mut self) -> Self {
        if self.0 & 0x80 != 0 {
            self.0 += 1 << 8;
        }

        self.0 &= !0xff;

        self
    }

    #[must_use]
    pub const fn from_samples_f(samples: f32, meter: &Meter) -> Self {
        let samples = samples as f64;
        let bpm = meter.bpm as f64;
        let sample_rate = meter.sample_rate as f64;

        let beat = samples * (bpm * 32.0) / (sample_rate * 15.0);

        Self(beat as u32)
    }

    #[must_use]
    pub const fn from_samples(samples: usize, meter: &Meter) -> Self {
        let samples = samples as u64;
        let bpm = meter.bpm as u64;
        let sample_rate = meter.sample_rate as u64;

        let beat = samples * (bpm * 32) / (sample_rate * 15);

        Self(beat as u32)
    }

    #[must_use]
    pub const fn in_samples_f(self, meter: &Meter) -> f32 {
        let beat = self.0 as f64;
        let bpm = meter.bpm as f64;
        let sample_rate = meter.sample_rate as f64;

        let samples = beat * (sample_rate * 15.0) / (bpm * 32.0);

        samples as f32
    }

    #[must_use]
    pub const fn in_samples(self, meter: &Meter) -> usize {
        let global_beat = self.0 as u64;
        let bpm = meter.bpm as u64;
        let sample_rate = meter.sample_rate as u64;

        let samples = global_beat * (sample_rate * 15) / (bpm * 32);

        samples as usize
    }

    #[must_use]
    pub fn floor_to_snap_step(mut self, scale: f32, meter: &Meter) -> Self {
        let snap_step = Self::snap_step(scale, meter).0;
        self.0 -= self.0 % snap_step;
        self
    }

    #[must_use]
    pub fn ceil_to_snap_step(mut self, scale: f32, meter: &Meter) -> Self {
        let snap_step = Self::snap_step(scale, meter).0;
        self.0 += snap_step - (self.0 % snap_step);
        self
    }

    #[must_use]
    pub fn round_to_snap_step(mut self, scale: f32, meter: &Meter) -> Self {
        let modulo = Self::snap_step(scale, meter).0;

        let diff = self.0 % modulo;

        if diff > modulo / 2 {
            self.0 += modulo - diff;
        } else {
            self.0 -= diff;
        }

        self
    }

    #[must_use]
    pub fn snap_step(mut scale: f32, meter: &Meter) -> Self {
        scale += (f32::from(meter.bpm) / 64.0).log2();

        Self(if scale < 12.0 {
            1 << (scale as u8 - 3)
        } else {
            u32::from(meter.numerator) << 8
        })
    }

    #[must_use]
    pub const fn saturating_sub(self, other: Self) -> Self {
        Self(self.0.saturating_sub(other.0))
    }

    #[must_use]
    pub const fn abs_diff(self, other: Self) -> Self {
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

impl Mul<u32> for Position {
    type Output = Self;

    fn mul(mut self, rhs: u32) -> Self::Output {
        self.0 *= rhs;
        self
    }
}

impl From<u32> for Position {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl From<Position> for u32 {
    fn from(value: Position) -> Self {
        value.0
    }
}
