use cpal::{
    BufferSize, SampleRate, StreamConfig,
    traits::{DeviceTrait as _, HostTrait as _},
};
use daw_ctx::DawCtx;
use std::sync::{
    Arc,
    atomic::Ordering::{AcqRel, Acquire},
};

mod audio_clip;
mod audio_track;
mod clip_position;
mod daw_ctx;
mod master;
mod meter;
mod midi_clip;
mod midi_track;
mod position;

pub use audio_clip::{AudioClip, InterleavedAudio, InterleavedAudioError, RubatoError, resample};
pub use audio_graph;
pub use audio_track::AudioTrack;
pub use clap_host;
pub use cpal::{Stream, traits::StreamTrait};
pub use daw_ctx::DawCtxMessage;
pub use master::Master;
pub use meter::{Denominator, Meter, Numerator};
pub use midi_clip::{MidiClip, MidiNote, MidiPattern};
pub use midi_track::MidiTrack;
pub use oneshot;
pub use position::Position;
pub use rtrb::{Consumer, Producer};
pub use strum::VariantArray as VARIANTS;

pub fn build_output_stream<T: Send + 'static>(
    sample_rate: u32,
    buffer_size: u32,
) -> (Stream, Producer<DawCtxMessage<T>>, Arc<Meter>) {
    let (mut ctx, producer) = DawCtx::create(sample_rate, buffer_size);
    let meter = ctx.meter.clone();

    let stream = cpal::default_host()
        .default_output_device()
        .unwrap()
        .build_output_stream(
            &StreamConfig {
                channels: 2,
                sample_rate: SampleRate(sample_rate),
                buffer_size: BufferSize::Fixed(buffer_size),
            },
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
