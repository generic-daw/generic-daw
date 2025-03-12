use cpal::{
    BufferSize, SampleRate, StreamConfig, SupportedBufferSize, SupportedStreamConfigRange,
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

    let device = cpal::default_host().default_output_device().unwrap();

    let supported_config = device
        .supported_output_configs()
        .unwrap()
        .filter(|config| config.channels() == 2)
        .filter(|config| config.max_sample_rate().0 >= 40000)
        .min_by(|l, r| compare_device_orderings(l, r, sample_rate, buffer_size))
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

fn compare_device_orderings(
    l: &SupportedStreamConfigRange,
    r: &SupportedStreamConfigRange,
    sample_rate: u32,
    buffer_size: u32,
) -> Ordering {
    let l_sample_rate = sample_rate
        .clamp(l.min_sample_rate().0, l.max_sample_rate().0)
        .abs_diff(sample_rate);
    let r_sample_rate = sample_rate
        .clamp(l.min_sample_rate().0, l.max_sample_rate().0)
        .abs_diff(sample_rate);

    if l_sample_rate != r_sample_rate {
        return l_sample_rate.cmp(&r_sample_rate);
    }

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
