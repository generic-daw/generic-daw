use atomig::{Atom, Atomic};
use std::{
    fmt::Display,
    sync::atomic::{AtomicBool, AtomicU16, AtomicU32, AtomicUsize, Ordering::SeqCst},
};
use strum::VariantArray;

#[repr(u8)]
#[derive(Atom, Clone, Copy, Debug, Default, Eq, PartialEq, VariantArray)]
pub enum Numerator {
    _1 = 1,
    _2 = 2,
    _3 = 3,
    #[default]
    _4 = 4,
    _5 = 5,
    _6 = 6,
    _7 = 7,
    _8 = 8,
    _9 = 9,
    _10 = 10,
    _11 = 11,
    _12 = 12,
    _13 = 13,
    _14 = 14,
    _15 = 15,
    _16 = 16,
}

impl Display for Numerator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", *self as u16)
    }
}

#[repr(u8)]
#[derive(Atom, Clone, Copy, Debug, Default, Eq, PartialEq, VariantArray)]
pub enum Denominator {
    _2 = 2,
    #[default]
    _4 = 4,
    _8 = 8,
    _16 = 16,
}

impl Display for Denominator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", *self as u16)
    }
}

#[derive(Debug)]
pub struct Meter {
    /// BPM of the arrangement, between 30 and 600
    pub bpm: AtomicU16,
    /// numerator of the time signature
    pub numerator: Atomic<Numerator>,
    /// denominator of the time signature
    pub denominator: Atomic<Denominator>,
    /// sample rate of the output stream
    ///
    /// typical values: 32000, 44100, 48000, 88200, 96000, 176400, 192000
    pub sample_rate: AtomicU32,
    /// whether the arrangement is currently being played back
    pub playing: AtomicBool,
    /// whether the arrangement is currently being exported
    ///
    /// this is a workaround to stop the output stream from starting playback while exporting
    pub exporting: AtomicBool,
    /// the current global time of the playhead, in samples
    pub sample: AtomicUsize,
}

impl Default for Meter {
    fn default() -> Self {
        Self {
            bpm: AtomicU16::new(140),
            numerator: Atomic::default(),
            denominator: Atomic::default(),
            sample_rate: AtomicU32::default(),
            playing: AtomicBool::default(),
            exporting: AtomicBool::default(),
            sample: AtomicUsize::default(),
        }
    }
}

impl Meter {
    pub fn reset(&self) {
        self.bpm.store(140, SeqCst);
        self.numerator.store(Numerator::default(), SeqCst);
        self.denominator.store(Denominator::default(), SeqCst);
    }
}
