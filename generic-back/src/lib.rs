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

// Function to get the default audio output device and its configuration
pub fn cpal_get_default_device() -> (Device, StreamConfig) {
    // Get the default output device
    let device = cpal::default_host()
        .default_output_device()
        .expect("no output device available");

    // Get the supported output configuration for stereo channels
    let supported_config = device
        .default_output_config()
        .expect("no output config available");

    (device, supported_config.into())
}

// Function to create an output stream
pub fn get_output_stream(
    device: &Device,
    config: &StreamConfig,
    audio: Arc<Mutex<Arrangement>>,
    play_from: u32,
) -> Stream {
    let mut global_time = play_from;

    // Build the output stream
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

// Function to convert seconds to interleaved samples
pub fn seconds_to_interleaved_samples(seconds: f32, sample_rate: u32) -> i32 {
    (seconds * sample_rate as f32) as i32 * 2 // Multiply by SAMPLE_RATE and by 2 (for interleaved stereo)
}
