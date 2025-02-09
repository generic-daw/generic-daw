use atomig::Atomic;
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicUsize};

mod denominator;
mod numerator;

pub use denominator::Denominator;
pub use numerator::Numerator;

#[derive(Debug)]
pub struct Meter {
    /// sample rate of the output stream
    ///
    /// typical values: 32000, 44100, 48000, 88200, 96000, 176400, 192000
    pub sample_rate: u32,
    /// BPM of the arrangement, between 30 and 600
    pub bpm: AtomicU16,
    /// numerator of the time signature
    pub numerator: Atomic<Numerator>,
    /// denominator of the time signature
    pub denominator: Atomic<Denominator>,
    /// whether the arrangement is currently being played back
    pub playing: AtomicBool,
    /// whether the metronome is currently enabled
    pub metronome: AtomicBool,
    /// the current global time of the playhead, in samples
    pub sample: AtomicUsize,
}

impl Meter {
    pub(crate) fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate,
            bpm: AtomicU16::new(140),
            numerator: Atomic::default(),
            denominator: Atomic::default(),
            playing: AtomicBool::default(),
            metronome: AtomicBool::default(),
            sample: AtomicUsize::default(),
        }
    }
}
