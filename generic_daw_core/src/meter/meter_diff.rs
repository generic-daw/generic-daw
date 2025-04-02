use super::{Denominator, Numerator};

#[derive(Clone, Copy, Debug, Default)]
pub struct MeterDiff {
    /// sample rate of the output stream
    ///
    /// typical values: 32000, 44100, 48000, 88200, 96000, 176400, 192000
    pub sample_rate: Option<u32>,
    /// buffer size of the output stream
    ///
    /// typically a power of two
    pub buffer_size: Option<u32>,
    /// BPM of the arrangement, in the `30..=600` range
    pub bpm: Option<u16>,
    /// numerator of the time signature
    pub numerator: Option<Numerator>,
    /// denominator of the time signature
    pub denominator: Option<Denominator>,
    /// whether the arrangement is currently being played back
    pub playing: Option<bool>,
    /// whether the metronome is currently enabled
    pub metronome: Option<bool>,
    /// the current global time of the playhead, in samples
    pub sample: Option<usize>,
}
