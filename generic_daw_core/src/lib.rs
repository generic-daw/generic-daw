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
mod delay_compensation_node;
mod master;
mod meter;
mod midi_clip;
mod midi_track;
mod mixer_node;
mod position;

pub use audio_clip::{AudioClip, InterleavedAudio, InterleavedAudioError, RubatoError, resample};
pub use audio_graph;
pub use audio_track::AudioTrack;
pub use clap_host;
pub use cpal::{Stream, traits::StreamTrait};
pub use daw_ctx::DawCtxMessage;
pub use delay_compensation_node::DelayCompensationNode;
pub use master::Master;
pub use meter::{Denominator, Meter, Numerator};
pub use midi_clip::{MidiClip, MidiNote, NoteId};
pub use midi_track::MidiTrack;
pub use mixer_node::MixerNode;
pub use oneshot;
pub use position::Position;
pub use rtrb::{Consumer, Producer};
pub use strum::VariantArray as VARIANTS;

pub fn build_output_stream(
    sample_rate: u32,
    buffer_size: u32,
) -> (Stream, Arc<MixerNode>, Producer<DawCtxMessage>, Arc<Meter>) {
    let (mut ctx, master_node, producer) = DawCtx::create(sample_rate, buffer_size);
    let meter = ctx.meter.clone();

    let device = cpal::default_host()
        .default_output_device()
        .expect("No output device available");

    let supported_configs = device
        .supported_output_configs()
        .expect("Error querying supported configs");

    let configs: Vec<_> = supported_configs
        .filter(|config| config.channels() == 2)
        .collect();

    let best_supported_config = configs.iter().find_map(|r| {
        let min = r.min_sample_rate().0;
        let max = r.max_sample_rate().0;

        if sample_rate >= min && sample_rate <= max {
            Some(r.with_sample_rate(SampleRate(sample_rate)))
        } else {
            None
        }
    });

    let supported_config = best_supported_config.unwrap_or_else(|| {
        let closest_config = configs
            .iter()
            .min_by_key(|r| {
                let mid_rate = (r.min_sample_rate().0 + sample_rate) / 2;
                if mid_rate > sample_rate {
                    mid_rate - sample_rate
                } else {
                    sample_rate - mid_rate
                }
            })
            .expect("No supported config found");

        let actual_rate =
            if closest_config.min_sample_rate().0 == closest_config.max_sample_rate().0 {
                closest_config.min_sample_rate().0
            } else {
                (closest_config.min_sample_rate().0 + closest_config.max_sample_rate().0) / 2
            };

        closest_config.with_sample_rate(SampleRate(actual_rate))
    });

    let config = StreamConfig {
        channels: supported_config.channels(),
        sample_rate: supported_config.sample_rate(),
        buffer_size: BufferSize::Fixed(supported_config.channels().min(buffer_size.try_into().unwrap()).into()),
    };

    let stream = device
        .build_output_stream(
            &config,
            move |data, _| {
                if ctx.meter.playing.load(Acquire) {
                    ctx.meter.sample.fetch_add(data.len(), AcqRel);
                }

                ctx.fill_buf(data);

                for s in data {
                    *s = s.clamp(-1.0, 1.0);
                }
            },
            |err| panic!("{err}"),
            None,
        )
        .expect("Failed to build output stream");

    stream.play().expect("Failed to play stream");

    stream.play().unwrap();

    (stream, master_node, producer, meter)
}
