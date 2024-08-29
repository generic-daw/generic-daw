pub mod arrangement;
pub mod clap_host;
pub mod track;
pub mod track_clip;

use arrangement::Arrangement;
use cpal::{
    traits::{DeviceTrait, HostTrait},
    Device, Stream, StreamConfig,
};
use std::sync::{mpsc::Receiver, Arc, Mutex};

pub fn cpal_get_default_device() -> (Device, StreamConfig) {
    let device = cpal::default_host()
        .default_output_device()
        .expect("no output device available");

    let supported_config = device
        .default_output_config()
        .expect("no output config available");

    (device, supported_config.into())
}

pub enum StreamMessage {
    Play,
    Pause,
    Stop,
    Jump(u32),
}

pub fn get_output_stream(
    device: &Device,
    config: &StreamConfig,
    audio: Arc<Mutex<Arrangement>>,
    receiver: Receiver<StreamMessage>,
) -> Stream {
    let mut global_time = 0;
    let mut playing = false;

    device
        .build_output_stream(
            config,
            move |data, _| {
                for sample in data.iter_mut() {
                    *sample = audio.lock().unwrap().get_at_global_time(global_time);

                    match receiver.try_recv() {
                        Ok(StreamMessage::Play) => {
                            playing = true;
                        }
                        Ok(StreamMessage::Pause) => {
                            playing = false;
                        }
                        Ok(StreamMessage::Stop) => {
                            playing = false;
                            global_time = 0;
                        }
                        Ok(StreamMessage::Jump(time)) => {
                            global_time = time;
                        }
                        _ => {}
                    }

                    if playing {
                        global_time += 1;
                    }
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
    (seconds * sample_rate as f32) as i32 * 2
}
