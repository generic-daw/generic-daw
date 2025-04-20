use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU16, AtomicUsize};

#[derive(Debug)]
pub struct Meter {
    /// sample rate of the output stream
    ///
    /// typical values: 32000, 44100, 48000, 88200, 96000, 176400, 192000
    pub sample_rate: u32,
    /// buffer size of the output stream
    ///
    /// typically a power of two
    pub buffer_size: u32,
    /// BPM of the arrangement, in the `30..=600` range
    pub bpm: AtomicU16,
    /// numerator of the time signature
    pub numerator: AtomicU8,
    /// whether the arrangement is currently being played back
    pub playing: AtomicBool,
    /// whether the metronome is currently enabled
    pub metronome: AtomicBool,
    /// the current global time of the playhead, in samples
    pub sample: AtomicUsize,
}

impl Meter {
    pub(crate) fn new(sample_rate: u32, buffer_size: u32) -> Self {
        Self {
            sample_rate,
            buffer_size,
            bpm: AtomicU16::new(140),
            numerator: AtomicU8::new(4),
            playing: AtomicBool::default(),
            metronome: AtomicBool::default(),
            sample: AtomicUsize::default(),
        }
    }
}
