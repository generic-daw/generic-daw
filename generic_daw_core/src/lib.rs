use cpal::{
    traits::{DeviceTrait as _, HostTrait as _},
    StreamConfig,
};
use daw_ctx::DawCtx;
use std::sync::{
    atomic::Ordering::{AcqRel, Acquire},
    Arc,
};

mod audio_clip;
mod audio_track;
mod daw_ctx;
mod master;
mod meter;
mod midi_clip;
mod midi_track;
mod position;

pub use audio_clip::{resample, AudioClip, InterleavedAudio, InterleavedAudioError, RubatoError};
pub use audio_graph;
pub use audio_track::AudioTrack;
pub use clap_host;
pub use cpal::{traits::StreamTrait, Stream};
pub use daw_ctx::DawCtxMessage;
pub use master::Master;
pub use meter::{Denominator, Meter, Numerator};
pub use midi_clip::{MidiClip, MidiNote, MidiPattern};
pub(crate) use midi_track::DirtyEvent;
pub use midi_track::MidiTrack;
pub use position::Position;
pub use rtrb::{Consumer, Producer};
pub use strum::VariantArray as VARIANTS;

pub fn build_output_stream<T: Send + 'static>() -> (Stream, Producer<DawCtxMessage<T>>, Arc<Meter>)
{
    let device = cpal::default_host().default_output_device().unwrap();
    let config: &StreamConfig = &device.default_output_config().unwrap().into();

    let (mut ctx, producer) = DawCtx::create(config.sample_rate.0);
    let meter = ctx.meter.clone();

    let stream = device
        .build_output_stream(
            config,
            move |data, _| {
                let sample = if ctx.meter.playing.load(Acquire) {
                    ctx.meter.sample.fetch_add(data.len(), AcqRel)
                } else {
                    ctx.meter.sample.load(Acquire)
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

    (stream, producer, meter)
}
