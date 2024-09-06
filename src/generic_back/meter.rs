use std::sync::{
    atomic::{AtomicBool, AtomicU32},
    Arc,
};

pub struct Meter {
    pub bpm: f64,
    pub numerator: u8,
    pub denominator: u8,
    pub sample_rate: u32,
    pub playing: Arc<AtomicBool>,
    pub exporting: Arc<AtomicBool>,
    pub global_time: Arc<AtomicU32>,
}

impl Meter {
    pub fn new(bpm: f64, numerator: u8, denominator: u8) -> Self {
        assert_eq!(denominator.count_ones(), 1);

        Self {
            bpm,
            numerator,
            denominator,
            sample_rate: 0,
            playing: Arc::new(AtomicBool::new(false)),
            exporting: Arc::new(AtomicBool::new(false)),
            global_time: Arc::new(AtomicU32::new(0)),
        }
    }
}

pub fn seconds_to_interleaved_samples(seconds: f64, meter: &Meter) -> u32 {
    let samples = (seconds * f64::from(meter.sample_rate) * 2.0).floor();
    assert!(samples <= f64::from(u32::MAX));
    samples as u32
}
