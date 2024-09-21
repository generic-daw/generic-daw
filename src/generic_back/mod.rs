mod arrangement;
pub use arrangement::Arrangement;

mod meter;
pub use meter::Meter;

mod position;
pub use position::Position;

mod track;
pub use track::{AudioTrack, MidiTrack, Track};

mod track_clip;
pub(in crate::generic_back) use track_clip::{AtomicDirtyEvent, DirtyEvent};
pub use track_clip::{AudioClip, InterleavedAudio, MidiClip, MidiNote};

use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Stream, StreamConfig,
};
use std::{
    f32::consts::PI,
    sync::{atomic::Ordering::SeqCst, Arc},
};

pub fn build_output_stream(arrangement: Arc<Arrangement>) -> Stream {
    let device = cpal::default_host().default_output_device().unwrap();
    let config: &StreamConfig = &device.default_output_config().unwrap().into();
    arrangement
        .meter
        .sample_rate
        .store(config.sample_rate.0, SeqCst);

    let stream = device
        .build_output_stream(
            config,
            move |data, _| {
                for sample in data.iter_mut() {
                    *sample =
                        arrangement.get_at_global_time(arrangement.meter.global_time.load(SeqCst));

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
        .unwrap();
    stream.play().unwrap();

    stream
}

pub fn seconds_to_interleaved_samples(seconds: f64, meter: &Meter) -> u32 {
    (seconds * f64::from(meter.sample_rate.load(SeqCst) * 2)) as u32
}

pub fn pan(angle: f32, global_time: u32) -> f32 {
    let angle = angle.mul_add(0.5, 0.5) * PI * 0.5;
    if global_time % 2 == 0 {
        angle.cos()
    } else {
        angle.sin()
    }
}
