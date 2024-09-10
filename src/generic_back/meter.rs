use std::sync::{
    atomic::{AtomicBool, AtomicU32},
    Arc,
};

pub struct Meter {
    pub bpm: u32,
    pub numerator: u32,
    /// this isn't actually the denominator
    /// get the actual denominator with `1 << denominator`
    pub denominator: u32,
    pub sample_rate: u32,
    pub playing: Arc<AtomicBool>,
    pub exporting: Arc<AtomicBool>,
    pub global_time: Arc<AtomicU32>,
}

impl Meter {
    pub fn new(bpm: u32, numerator: u32, denominator: u32) -> Self {
        assert_eq!(denominator.count_ones(), 1);

        Self {
            bpm,
            numerator,
            denominator: denominator.trailing_zeros(),
            sample_rate: 0,
            playing: Arc::new(AtomicBool::new(false)),
            exporting: Arc::new(AtomicBool::new(false)),
            global_time: Arc::new(AtomicU32::new(0)),
        }
    }
}

pub fn seconds_to_interleaved_samples(seconds: f64, meter: &Meter) -> u32 {
    (seconds * f64::from(meter.sample_rate) * 2.0) as u32
}
