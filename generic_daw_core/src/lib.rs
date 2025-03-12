use cpal::{
    BufferSize, SampleRate, StreamConfig, SupportedBufferSize,
    traits::{DeviceTrait as _, HostTrait as _},
};
use daw_ctx::DawCtx;
use std::{
    cmp::Ordering,
    sync::{
        Arc,
        atomic::Ordering::{AcqRel, Acquire},
    },
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

    let mut configs: Box<[_]> = supported_configs
        .filter(|config| config.channels() == 2)
        .collect();

    configs.sort_unstable_by(|l, r| match (*l.buffer_size(), *r.buffer_size()) {
        (SupportedBufferSize::Unknown, SupportedBufferSize::Unknown) => Ordering::Equal,
        (SupportedBufferSize::Range { .. }, SupportedBufferSize::Unknown) => Ordering::Less,
        (SupportedBufferSize::Unknown, SupportedBufferSize::Range { .. }) => Ordering::Greater,
        (SupportedBufferSize::Range { min, max }, _) if (min..=max).contains(&buffer_size) => {
            Ordering::Less
        }
        (_, SupportedBufferSize::Range { min, max }) if (min..=max).contains(&buffer_size) => {
            Ordering::Greater
        }
        (
            SupportedBufferSize::Range {
                min: lmin,
                max: lmax,
            },
            SupportedBufferSize::Range {
                min: rmin,
                max: rmax,
            },
        ) => {
            let ldiff = lmin.abs_diff(sample_rate).min(lmax.abs_diff(sample_rate));
            let rdiff = rmin.abs_diff(sample_rate).min(rmax.abs_diff(sample_rate));
            ldiff.cmp(&rdiff)
        }
    });

    let supported_config = configs
        .into_iter()
        .min_by_key(|config| {
            let min = config.min_sample_rate().0;
            let max = config.max_sample_rate().0;
            sample_rate.clamp(min, max).abs_diff(sample_rate)
        })
        .expect("No supported config found");

    let sample_rate = SampleRate({
        let min = supported_config.min_sample_rate().0;
        let max = supported_config.max_sample_rate().0;
        sample_rate.clamp(min, max)
    });

    let buffer_size = match *supported_config.buffer_size() {
        SupportedBufferSize::Unknown => BufferSize::Default,
        SupportedBufferSize::Range { min, max } => BufferSize::Fixed(buffer_size.clamp(min, max)),
    };

    let stream = device
        .build_output_stream(
            &StreamConfig {
                channels: 2,
                sample_rate,
                buffer_size,
            },
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
