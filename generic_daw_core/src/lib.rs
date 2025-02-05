use audio_ctx::AudioCtx;
use cpal::{
    traits::{DeviceTrait as _, HostTrait as _, StreamTrait as _},
    Stream, StreamConfig,
};
use master::Master;
use rtrb::{Consumer, Producer};
use std::sync::{
    atomic::Ordering::{AcqRel, Acquire},
    Arc,
};

mod audio_ctx;
mod denominator;
mod live_sample;
mod master;
mod meter;
mod numerator;
mod position;
mod track;
mod track_clip;

pub use audio_ctx::{AudioCtxMessage, UiMessage};
pub use audio_graph;
pub use clap_host;
pub use cpal;
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

#[expect(clippy::type_complexity)]
pub fn build_output_stream<T: Send + 'static>() -> (
    Stream,
    Producer<AudioCtxMessage<T>>,
    Consumer<UiMessage<T>>,
    Arc<Meter>,
) {
    let device = cpal::default_host().default_output_device().unwrap();
    let config: &StreamConfig = &device.default_output_config().unwrap().into();

    let arrangement = Master::new(config.sample_rate.0);
    let meter = arrangement.meter.clone();
    let (mut ctx, producer, consumer) = AudioCtx::create(arrangement.into(), meter.clone());

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

    (stream, producer, consumer, meter)
}
