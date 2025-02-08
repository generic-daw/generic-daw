use atomig::Atomic;
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, AtomicUsize};

mod denominator;
mod numerator;

pub use denominator::Denominator;
pub use numerator::Numerator;

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
    /// whether the metronome is currently enabled
    pub metronome: AtomicBool,
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
            metronome: AtomicBool::default(),
            sample: AtomicUsize::default(),
        }
    }
}

impl Meter {
    pub(crate) fn new(sample_rate: u32) -> Self {
        Self {
            bpm: AtomicU16::new(140),
            numerator: Atomic::default(),
            denominator: Atomic::default(),
            sample_rate: AtomicU32::new(sample_rate),
            playing: AtomicBool::default(),
            metronome: AtomicBool::default(),
            sample: AtomicUsize::default(),
        }
    }
}
