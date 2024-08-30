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

    pub fn to_interleaved_samples(&self, meter: &Meter, sample_rate: u32) -> u32 {
        let global_beat = f64::from(self.quarter_note * u32::from(meter.denominator)) / 4.0
            + f64::from(self.sub_quarter_note) / 256.0;

        seconds_to_interleaved_samples(global_beat * meter.bpm / 60.0, sample_rate)
    }
}

pub fn seconds_to_interleaved_samples(seconds: f64, sample_rate: u32) -> u32 {
    let samples = (seconds * f64::from(sample_rate) * 2f64).floor();
    assert!(samples <= f64::from(u32::MAX));
    samples as u32
}

#[derive(PartialEq)]
pub struct Meter {
    bpm: f64,
    numerator: u8,
    denominator: u8,
}

impl Meter {
    fn new(bpm: f64, numerator: u8, denominator: u8) -> Self {
        assert_eq!(denominator.count_ones(), 1);

        Self {
            bpm,
            numerator,
            denominator,
        }
    }
}
