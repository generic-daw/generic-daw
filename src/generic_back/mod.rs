use cpal::{
    traits::{DeviceTrait as _, HostTrait as _, StreamTrait as _},
    Stream, StreamConfig,
};
use std::{
    f32::consts::PI,
    sync::{atomic::Ordering::SeqCst, Arc},
};
use track::DirtyEvent;

pub use arrangement::Arrangement;
pub use live_sample::LiveSample;
pub use meter::{Denominator, Meter, Numerator};
pub use position::Position;
pub use track::{AudioTrack, MidiTrack, Track};
pub use track_clip::{resample, AudioClip, InterleavedAudio, MidiNote, TrackClip};

mod arrangement;
mod live_sample;
mod meter;
mod position;
mod track;
mod track_clip;

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
                let sample = if arrangement.meter.playing.load(SeqCst) {
                    arrangement.meter.sample.fetch_add(data.len(), SeqCst)
                } else {
                    arrangement.meter.sample.load(SeqCst)
                };

                for s in data.iter_mut() {
                    *s = 0.0;
                }

                arrangement.fill_buf(sample, data);

                for s in data {
                    *s = s.clamp(-1.0, 1.0);
                }
            },
            move |err| panic!("{}", err),
            None,
        )
        .unwrap();
    stream.play().unwrap();

    stream
}

pub fn seconds_to_interleaved_samples(seconds: f32, meter: &Meter) -> f32 {
    seconds * meter.sample_rate.load(SeqCst) as f32 * 2.0
}

pub fn pan(angle: f32) -> (f32, f32) {
    let angle = angle.mul_add(0.5, 0.5) * PI * 0.5;

    (angle.cos(), angle.sin())
}
