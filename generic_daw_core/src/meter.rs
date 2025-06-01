#[derive(Clone, Copy, Debug)]
pub struct Meter {
    /// sample rate of the output stream
    ///
    /// typical values: 44100, 48000, 88200, 96000, 176400, 192000
    pub sample_rate: u32,
    /// buffer size of the output stream
    ///
    /// typically a power of two
    pub buffer_size: u32,
    /// BPM of the arrangement, in the `30..=600` range
    pub bpm: u16,
    /// numerator of the time signature
    pub numerator: u8,
    /// whether the arrangement is currently being played back
    pub playing: bool,
    /// whether the metronome is currently enabled
    pub metronome: bool,
    /// the current global time of the playhead, in samples
    pub sample: usize,
}

impl Meter {
    pub(crate) fn new(sample_rate: u32, buffer_size: u32) -> Self {
        Self {
            sample_rate,
            buffer_size,
            bpm: 140,
            numerator: 4,
            playing: false,
            metronome: false,
            sample: 0,
        }
    }
}
