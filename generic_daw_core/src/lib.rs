use async_channel::Receiver;
use cpal::{
    BufferSize, SampleRate, StreamConfig, SupportedBufferSize, SupportedStreamConfigRange,
    traits::{DeviceTrait as _, HostTrait as _},
};
use daw_ctx::DawCtx;
use rtrb::Producer;
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

pub use audio_clip::{
    AudioClip, InterleavedAudio, InterleavedAudioError, RubatoError, resample_interleaved,
    resample_planar,
};
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
pub use position::Position;
pub use strum::VariantArray as VARIANTS;

pub fn build_input_stream(sample_rate: u32) -> (u16, u32, Stream, Receiver<Box<[f32]>>) {
    let (sender, receiver) = async_channel::unbounded();

    let device = cpal::default_host().default_input_device().unwrap();

    let supported_config = device
        .supported_input_configs()
        .unwrap()
        .filter(|config| config.max_sample_rate().0 >= 40000)
        .min_by(|l, r| compare_by_sample_rate(l, r, sample_rate))
        .unwrap();

    let sample_rate = SampleRate(sample_rate.clamp(
        supported_config.min_sample_rate().0,
        supported_config.max_sample_rate().0,
    ));

    let stream = device
        .build_input_stream(
            &supported_config.with_sample_rate(sample_rate).config(),
            move |data, _| sender.try_send(data.into()).unwrap(),
            |err| panic!("{err}"),
            None,
        )
        .unwrap();

    stream.play().unwrap();

    (supported_config.channels(), sample_rate.0, stream, receiver)
}

pub fn build_output_stream(
    sample_rate: u32,
    buffer_size: u32,
) -> (Stream, Arc<MixerNode>, Producer<DawCtxMessage>, Arc<Meter>) {
    let (mut ctx, master_node, producer) = DawCtx::create(sample_rate, buffer_size);
    let meter = ctx.meter.clone();

    let device = cpal::default_host().default_output_device().unwrap();

    let supported_config = device
        .supported_output_configs()
        .unwrap()
        .filter(|config| config.channels() == 2)
        .filter(|config| config.max_sample_rate().0 >= 40000)
        .min_by(|l, r| {
            compare_by_sample_rate(l, r, sample_rate)
                .then_with(|| compare_by_buffer_size(l, r, buffer_size))
        })
        .unwrap();

    let sample_rate = SampleRate(sample_rate.clamp(
        supported_config.min_sample_rate().0,
        supported_config.max_sample_rate().0,
    ));

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
        .unwrap();

    stream.play().unwrap();

    (stream, master_node, producer, meter)
}

fn compare_by_sample_rate(
    l: &SupportedStreamConfigRange,
    r: &SupportedStreamConfigRange,
    sample_rate: u32,
) -> Ordering {
    let ldiff = sample_rate
        .clamp(l.min_sample_rate().0, l.max_sample_rate().0)
        .abs_diff(sample_rate);
    let rdiff = sample_rate
        .clamp(r.min_sample_rate().0, r.max_sample_rate().0)
        .abs_diff(sample_rate);

    ldiff.cmp(&rdiff)
}

fn compare_by_buffer_size(
    l: &SupportedStreamConfigRange,
    r: &SupportedStreamConfigRange,
    buffer_size: u32,
) -> Ordering {
    match (*l.buffer_size(), *r.buffer_size()) {
        (SupportedBufferSize::Unknown, SupportedBufferSize::Unknown) => Ordering::Equal,
        (SupportedBufferSize::Range { .. }, SupportedBufferSize::Unknown) => Ordering::Less,
        (SupportedBufferSize::Unknown, SupportedBufferSize::Range { .. }) => Ordering::Greater,
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
            let ldiff = buffer_size.clamp(lmin, lmax).abs_diff(buffer_size);
            let rdiff = buffer_size.clamp(rmin, rmax).abs_diff(buffer_size);
            ldiff.cmp(&rdiff)
        }
    }
}
