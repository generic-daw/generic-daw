pub mod arrangement;
pub mod clap_host;
pub mod track;
pub mod track_clip;

use arrangement::Arrangement;
use cpal::{
    traits::{DeviceTrait, HostTrait},
    Device, Stream, StreamConfig,
};
use std::sync::{Arc, Mutex};

pub fn cpal_get_default_device() -> (Device, StreamConfig) {
    let device = cpal::default_host()
        .default_output_device()
        .expect("no output device available");

    let supported_config = device
        .default_output_config()
        .expect("no output config available");

    (device, supported_config.into())
}

pub fn get_output_stream(
    device: &Device,
    config: &StreamConfig,
    audio: Arc<Mutex<Arrangement>>,
    play_from: u32,
) -> Stream {
    let mut global_time = play_from;

    device
        .build_output_stream(
            config,
            move |data, _| {
                for sample in data.iter_mut() {
                    *sample = audio.lock().unwrap().get_at_global_time(global_time);
                    global_time += 1;
                }
            },
            move |err| {
                eprintln!("an error occurred on stream: {err}");
            },
            None,
        )
        .unwrap()
}

pub fn seconds_to_interleaved_samples(seconds: f32, sample_rate: u32) -> i32 {
    (seconds * sample_rate as f32) as i32 * 2 // Multiply by SAMPLE_RATE and by 2 (for interleaved stereo)
}
