use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, AtomicU8};

#[derive(Debug)]
pub struct Meter {
    /// BPM of the arrangement, between 30 and 600
    pub bpm: AtomicU16,
    // numerator of the time signature, between 1 and 255
    pub numerator: AtomicU8,
    /// log2 of the denominator of the time signature, between 0 and 7
    ///
    /// get the actual denominator with `1 << denominator`
    pub denominator: AtomicU8,
    /// sample rate of the output stream
    ///
    /// typical values: 44100, 48000, 88200, 96000, 176400, 192000
    pub sample_rate: AtomicU32,
    /// whether the arrangement is currently being played back
    pub playing: AtomicBool,
    /// whether the arrangement is currently being exported
    ///
    /// this is a workaround to stop the output stream from starting playback while exporting
    pub exporting: AtomicBool,
    /// the current global time of the playhead, in samples
    pub global_time: AtomicU32,
}

impl Default for Meter {
    fn default() -> Self {
        Self {
            bpm: AtomicU16::new(140),
            numerator: AtomicU8::new(4),
            denominator: AtomicU8::new(2),
            sample_rate: AtomicU32::default(),
            playing: AtomicBool::default(),
            exporting: AtomicBool::default(),
            global_time: AtomicU32::default(),
        }
    }
}
