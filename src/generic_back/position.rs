use std::{
    ops::{Add, AddAssign, Sub, SubAssign},
    sync::Arc,
};

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct Position {
    quarter_note: u32,
    sub_quarter_note: u8,
}

impl Position {
    pub const fn new(quarter_note: u32, sub_quarter_note: u8) -> Self {
        Self {
            quarter_note,
            sub_quarter_note,
        }
    }

    pub fn from_interleaved_samples(samples: u32, meter: &Arc<Meter>) -> Self {
        let global_beat =
            f64::from(samples) / (f64::from(meter.sample_rate) * 2.0 * meter.bpm / 60.0);
        let quarter_note = global_beat as u32;
        let sub_quarter_note = ((global_beat - f64::from(quarter_note)) * 256.0) as u8;

        Self {
            quarter_note,
            sub_quarter_note,
        }
    }

    pub fn in_interleaved_samples(self, meter: &Arc<Meter>) -> u32 {
        let global_beat = f64::from(self.quarter_note * u32::from(meter.denominator)) / 4.0
            + f64::from(self.sub_quarter_note) / 256.0;

        seconds_to_interleaved_samples(global_beat * meter.bpm / 60.0, meter)
    }
}

impl PartialOrd for Position {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Position {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.quarter_note.cmp(&other.quarter_note) {
            std::cmp::Ordering::Equal => self.sub_quarter_note.cmp(&other.sub_quarter_note),
            other => other,
        }
    }
}

impl Add for Position {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        let new_sub_quarter_note =
            u32::from(self.sub_quarter_note) + u32::from(rhs.sub_quarter_note);
        Self {
            quarter_note: self.quarter_note + rhs.quarter_note + (new_sub_quarter_note / 256),
            sub_quarter_note: u8::try_from(new_sub_quarter_note % 256).unwrap(),
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
        let new_sub_quarter_note =
            i32::from(self.sub_quarter_note) - i32::from(rhs.sub_quarter_note);
        Self {
            quarter_note: u32::try_from(
                i32::try_from(self.quarter_note - rhs.quarter_note).unwrap()
                    + (new_sub_quarter_note / 256),
            )
            .unwrap(),
            sub_quarter_note: u8::try_from(new_sub_quarter_note % 256).unwrap(),
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

pub fn seconds_to_interleaved_samples(seconds: f64, meter: &Arc<Meter>) -> u32 {
    let samples = (seconds * f64::from(meter.sample_rate) * 2f64).floor();
    assert!(samples <= f64::from(u32::MAX));
    samples as u32
}

pub struct Meter {
    pub bpm: f64,
    pub numerator: u8,
    pub denominator: u8,
    pub sample_rate: u32,
}

impl Meter {
    pub fn new(bpm: f64, numerator: u8, denominator: u8, sample_rate: u32) -> Self {
        assert_eq!(denominator.count_ones(), 1);

        Self {
            bpm,
            numerator,
            denominator,
            sample_rate,
        }
    }
}
