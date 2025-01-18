use audio_graph::AudioGraphNodeImpl as _;
use cpal::{
    traits::{DeviceTrait as _, HostTrait as _, StreamTrait as _},
    StreamConfig,
};
use include_data::include_f32s;
use std::sync::{atomic::Ordering::SeqCst, Arc};

mod arrangement;
mod denominator;
mod live_sample;
mod meter;
mod numerator;
mod position;
mod track;
mod track_clip;

pub use arrangement::Arrangement;
pub use audio_graph;
pub use clap_host;
pub use cpal::Stream;
pub use denominator::Denominator;
pub use live_sample::LiveSample;
pub use meter::Meter;
pub use numerator::Numerator;
pub use position::Position;
pub(crate) use track::midi_track::dirty_event::DirtyEvent;
pub use track::{audio_track::AudioTrack, midi_track::MidiTrack, Track};
pub use track_clip::{
    audio_clip::{
        interleaved_audio::{resample, InterleavedAudio},
        AudioClip,
    },
    midi_clip::{midi_note::MidiNote, midi_pattern::MidiPattern, MidiClip},
    TrackClip,
};

static ON_BAR_CLICK: &[f32] = include_f32s!("../../assets/on_bar_click.pcm");
static OFF_BAR_CLICK: &[f32] = include_f32s!("../../assets/off_bar_click.pcm");

pub fn build_output_stream(arrangement: Arc<Arrangement>) -> Stream {
    let device = cpal::default_host().default_output_device().unwrap();
    let config: &StreamConfig = &device.default_output_config().unwrap().into();

    arrangement
        .meter
        .sample_rate
        .store(config.sample_rate.0, SeqCst);

    arrangement.on_bar_click.get_or_init(|| {
        resample(44100, config.sample_rate.0, ON_BAR_CLICK.into())
            .unwrap()
            .into()
    });
    arrangement.off_bar_click.get_or_init(|| {
        resample(44100, config.sample_rate.0, OFF_BAR_CLICK.into())
            .unwrap()
            .into()
    });

    let stream = device
        .build_output_stream(
            config,
            move |data, _| {
                let sample = if arrangement.meter.playing.load(SeqCst) {
                    arrangement.meter.sample.fetch_add(data.len(), SeqCst)
                } else {
                    arrangement.meter.sample.load(SeqCst)
                };

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

#[must_use]
pub fn seconds_to_interleaved_samples(seconds: f32, meter: &Meter) -> f32 {
    seconds * meter.sample_rate.load(SeqCst) as f32 * 2.0
}
