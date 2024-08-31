pub mod arrangement;
pub mod clap_host;
pub mod position;
pub mod track;
pub mod track_clip;

use arrangement::Arrangement;
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Device, Stream, StreamConfig,
};
use position::Meter;
use std::sync::{
    mpsc::{Receiver, Sender},
    Arc, Mutex,
};

pub enum StreamMessage {
    TogglePlay,
    Stop,
    Jump(u32),
}

pub struct DawStream {
    stream: Stream,
    playing: bool,
    sender: Sender<StreamMessage>,
}

impl DawStream {
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(audio: Arc<Mutex<Arrangement>>, meter: Arc<Meter>) -> Self {
        let device = cpal::default_host().default_output_device().unwrap();

        let config = device.default_output_config().unwrap().into();

        let (sender, receiver) = std::sync::mpsc::channel();

        let stream = get_output_stream(&device, &config, meter, audio, receiver);
        stream.play().unwrap();

        Self {
            stream,
            playing: false,
            sender,
        }
    }

    pub const fn playing(&self) -> bool {
        self.playing
    }

    pub fn toggle_play(&mut self) {
        self.playing ^= true;
        _ = self.sender.send(StreamMessage::TogglePlay);
    }

    pub fn stop(&mut self) {
        self.playing = false;
        _ = self.sender.send(StreamMessage::Stop);
    }

    pub fn jump(&self, time: u32) {
        self.sender.send(StreamMessage::Jump(time)).unwrap();
    }
}

fn get_output_stream(
    device: &Device,
    config: &StreamConfig,
    meter: Arc<Meter>,
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
                    *sample = audio
                        .lock()
                        .unwrap()
                        .get_at_global_time(global_time, &meter.clone());

                    match receiver.try_recv() {
                        Ok(StreamMessage::TogglePlay) => {
                            playing ^= true;
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
