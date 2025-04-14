use async_channel::{Receiver, Sender};
use audio_graph_node::AudioGraphNode;
use cpal::{
    BufferSize, SampleRate, StreamConfig, SupportedBufferSize, SupportedStreamConfigRange,
    traits::{DeviceTrait as _, HostTrait as _},
};
use daw_ctx::DawCtx;
use event::Event;
use log::info;
use std::{cmp::Ordering, sync::Arc};

mod audio_clip;
mod audio_graph_node;
mod clip;
mod clip_position;
mod daw_ctx;
mod decibels;
mod event;
mod export;
mod master;
mod meter;
mod midi_clip;
mod mixer_node;
mod position;
mod recording;
mod track;

pub use audio_clip::{AudioClip, InterleavedAudio};
pub(crate) use audio_clip::{resample_interleaved, resampler};
pub use audio_graph;
pub use clap_host;
pub use clip::Clip;
pub use cpal::{Stream, traits::StreamTrait};
pub use daw_ctx::DawCtxMessage;
pub use decibels::Decibels;
pub use export::export;
pub use master::Master;
pub use meter::{Meter, Numerator};
pub use midi_clip::{Key, MidiClip, MidiKey, MidiNote};
pub use mixer_node::MixerNode;
pub use position::Position;
pub use recording::Recording;
pub use track::Track;

type AudioGraph = audio_graph::AudioGraph<AudioGraphNode, Event>;

pub fn build_input_stream(
    sample_rate: u32,
    buffer_size: u32,
) -> (Stream, StreamConfig, Receiver<Box<[f32]>>) {
    let (sender, receiver) = async_channel::unbounded();

    let device = cpal::default_host().default_input_device().unwrap();

    let config = choose_config(
        device.supported_input_configs().unwrap(),
        sample_rate,
        buffer_size,
    );

    info!("starting output stream with config {config:?}",);

    let stream = device
        .build_input_stream(
            &config,
            move |data, _| sender.try_send(data.into()).unwrap(),
            |err| panic!("{err}"),
            None,
        )
        .unwrap();

    stream.play().unwrap();

    (stream, config, receiver)
}

pub fn build_output_stream(
    sample_rate: u32,
    buffer_size: u32,
) -> (Stream, Arc<MixerNode>, Sender<DawCtxMessage>, Arc<Meter>) {
    let (mut ctx, meter, node, sender) = DawCtx::create(sample_rate, buffer_size);

    let device = cpal::default_host().default_output_device().unwrap();

    let config = choose_config(
        device.supported_output_configs().unwrap(),
        sample_rate,
        buffer_size,
    );

    info!("starting output stream with config {config:?}");

    let stream = device
        .build_output_stream(
            &config,
            move |buf, _| ctx.process(buf),
            |err| panic!("{err}"),
            None,
        )
        .unwrap();

    stream.play().unwrap();

    (stream, node, sender, meter)
}

fn choose_config(
    configs: impl IntoIterator<Item = SupportedStreamConfigRange>,
    sample_rate: u32,
    buffer_size: u32,
) -> StreamConfig {
    let config = configs
        .into_iter()
        .filter(|config| config.channels() == 2)
        .filter(|config| config.max_sample_rate().0 >= 40000)
        .min_by(|l, r| {
            compare_by_sample_rate(l, r, sample_rate)
                .then_with(|| compare_by_buffer_size(l, r, buffer_size))
        })
        .unwrap();

    let sample_rate =
        SampleRate(sample_rate.clamp(config.min_sample_rate().0, config.max_sample_rate().0));

    let buffer_size = match *config.buffer_size() {
        SupportedBufferSize::Unknown => BufferSize::Default,
        SupportedBufferSize::Range { min, max } => BufferSize::Fixed(buffer_size.clamp(min, max)),
    };

    StreamConfig {
        channels: 2,
        sample_rate,
        buffer_size,
    }
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
