pub mod arrangement;
pub mod meter;
pub mod position;
pub mod track;
pub mod track_clip;

use arrangement::Arrangement;
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    StreamConfig,
};
use meter::Meter;
use std::{
    f32::consts::PI,
    ops::Rem,
    sync::{atomic::Ordering::SeqCst, Arc},
};

pub fn build_output_stream(arrangement: Arc<Arrangement>) {
    let device = cpal::default_host().default_output_device().unwrap();
    let config: &StreamConfig = &device.default_output_config().unwrap().into();
    arrangement
        .meter
        .sample_rate
        .store(config.sample_rate.0, SeqCst);

    let stream = Box::new(
        device
            .build_output_stream(
                config,
                move |data, _| {
                    for sample in data.iter_mut() {
                        *sample = arrangement
                            .get_at_global_time(arrangement.meter.global_time.load(SeqCst));
                        if arrangement.meter.playing.load(SeqCst)
                            && !arrangement.meter.exporting.load(SeqCst)
                        {
                            arrangement.meter.global_time.fetch_add(1, SeqCst);
                        }
                    }
                },
                move |err| panic!("{}", err),
                None,
            )
            .unwrap(),
    );
    stream.play().unwrap();
    Box::leak(stream);
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

fn gcd<T>(x: T, y: T) -> T
where
    T: Copy + PartialEq + PartialOrd + Rem<Output = T> + From<u8>,
{
    if y == 0.into() {
        x
    } else {
        let v = x % y;
        gcd(y, v)
    }
}
