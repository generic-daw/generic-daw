mod meter_diff;
mod time_signature;

pub use meter_diff::MeterDiff;
pub use time_signature::{Denominator, Numerator};

#[derive(Clone, Copy, Debug, Default)]
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
    pub bpm: u16,
    /// numerator of the time signature
    pub numerator: Numerator,
    /// denominator of the time signature
    pub denominator: Denominator,
    /// whether the arrangement is currently being played back
    pub playing: bool,
    /// whether the metronome is currently enabled
    pub metronome: bool,
    /// the current global time of the playhead, in samples
    pub sample: usize,
}

impl Meter {
    #[must_use]
    pub fn diff(self, other: Self) -> MeterDiff {
        MeterDiff {
            sample_rate: (self.sample_rate != other.sample_rate).then_some(other.sample_rate),
            buffer_size: (self.buffer_size != other.buffer_size).then_some(other.buffer_size),
            bpm: (self.bpm != other.bpm).then_some(other.bpm),
            numerator: (self.numerator != other.numerator).then_some(other.numerator),
            denominator: (self.denominator != other.denominator).then_some(other.denominator),
            playing: (self.playing != other.playing).then_some(other.playing),
            metronome: (self.metronome != other.metronome).then_some(other.metronome),
            sample: (self.sample != other.sample).then_some(other.sample),
        }
    }

    #[must_use]
    pub fn resolve(self, diff: MeterDiff) -> Self {
        Self {
            sample_rate: diff.sample_rate.unwrap_or(self.sample_rate),
            buffer_size: diff.buffer_size.unwrap_or(self.buffer_size),
            bpm: diff.bpm.unwrap_or(self.bpm),
            numerator: diff.numerator.unwrap_or(self.numerator),
            denominator: diff.denominator.unwrap_or(self.denominator),
            playing: diff.playing.unwrap_or(self.playing),
            metronome: diff.metronome.unwrap_or(self.metronome),
            sample: diff.sample.unwrap_or(self.sample),
        }
    }
}
