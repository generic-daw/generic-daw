use std::{
    f32::consts::PI,
    sync::atomic::{AtomicBool, AtomicU32, Ordering::SeqCst},
};

pub struct Meter {
    pub bpm: AtomicU32,
    pub numerator: AtomicU32,
    /// this isn't actually the denominator
    /// get the actual denominator with `1 << denominator`
    pub denominator: AtomicU32,
    pub sample_rate: AtomicU32,
    pub playing: AtomicBool,
    pub exporting: AtomicBool,
    pub global_time: AtomicU32,
}

impl Default for Meter {
    fn default() -> Self {
        Self::new()
    }
}

impl Meter {
    pub const fn new() -> Self {
        Self {
            bpm: AtomicU32::new(140),
            numerator: AtomicU32::new(4),
            denominator: AtomicU32::new(4),
            sample_rate: AtomicU32::new(0),
            playing: AtomicBool::new(false),
            exporting: AtomicBool::new(false),
            global_time: AtomicU32::new(0),
        }
    }
}

pub fn seconds_to_interleaved_samples(seconds: f64, meter: &Meter) -> u32 {
    (seconds * f64::from(meter.sample_rate.load(SeqCst)) * 2.0) as u32
}

pub fn pan(angle: f32, global_time: u32) -> f32 {
    let angle = angle.mul_add(0.5, 0.5) * PI * 0.5;
    if global_time % 2 == 0 {
        angle.cos()
    } else {
        angle.sin()
    }
}
