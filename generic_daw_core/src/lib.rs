use audio_ctx::AudioCtx;
use cpal::{
    traits::{DeviceTrait as _, HostTrait as _},
    StreamConfig,
};
use rtrb::Producer;
use std::sync::{
    atomic::Ordering::{AcqRel, Acquire},
    Arc,
};

mod arrangement;
mod audio_ctx;
mod denominator;
mod live_sample;
mod meter;
mod numerator;
mod position;
mod track;
mod track_clip;

pub use arrangement::Arrangement;
pub use audio_ctx::AudioCtxMessage;
pub use audio_graph;
pub use clap_host;
pub use cpal::{traits::StreamTrait, Stream};
pub use denominator::Denominator;
pub use live_sample::LiveSample;
pub use meter::Meter;
pub use numerator::Numerator;
pub use position::Position;
pub use rtrb;
pub(crate) use track::DirtyEvent;
pub use track::Track;
pub use track_clip::{
    audio_clip::{
        interleaved_audio::{resample, InterleavedAudio},
        AudioClip,
    },
    midi_clip::{midi_note::MidiNote, midi_pattern::MidiPattern, MidiClip},
    TrackClip,
};

pub fn build_output_stream() -> (Stream, Producer<AudioCtxMessage>, Arc<Arrangement>) {
    let device = cpal::default_host().default_output_device().unwrap();
    let config: &StreamConfig = &device.default_output_config().unwrap().into();

    let arrangement = Arc::new(Arrangement::new(config.sample_rate.0));
    let node = arrangement.clone().into();
    let (mut ctx, producer) = AudioCtx::create(node);
    let meter = arrangement.meter.clone();

    let stream = device
        .build_output_stream(
            config,
            move |data, _| {
                let sample = if meter.playing.load(Acquire) {
                    meter.sample.fetch_add(data.len(), AcqRel)
                } else {
                    meter.sample.load(Acquire)
                };

                ctx.fill_buf(sample, data);

                for s in data {
                    *s = s.clamp(-1.0, 1.0);
                }
            },
            |err| panic!("{err}"),
            None,
        )
        .unwrap();
    stream.play().unwrap();

    (stream, producer, arrangement)
}

#[must_use]
pub fn seconds_to_interleaved_samples(seconds: f32, meter: &Meter) -> f32 {
    seconds * meter.sample_rate.load(Acquire) as f32 * 2.0
}
