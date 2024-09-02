pub mod arrangement;
pub mod clap_host;
pub mod position;
pub mod track;
pub mod track_clip;

use arrangement::Arrangement;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use position::Meter;
use std::sync::{mpsc::Sender, Arc, RwLock};

pub enum StreamMessage {
    TogglePlay,
    Stop,
    Jump(u32),
}

#[allow(clippy::needless_pass_by_value)]
pub fn build_output_stream(
    arrangement: Arc<RwLock<Arrangement>>,
    meter: Arc<RwLock<Meter>>,
) -> Sender<StreamMessage> {
    let device = cpal::default_host().default_output_device().unwrap();

    let config = &device.default_output_config().unwrap().into();

    let (sender, receiver) = std::sync::mpsc::channel();

    let mut global_time = 0;
    let mut playing = false;
    let stream = Box::new(
        device
            .build_output_stream(
                config,
                move |data, _| {
                    for sample in data.iter_mut() {
                        *sample = arrangement
                            .read()
                            .unwrap()
                            .get_at_global_time(global_time, &meter);

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
            .unwrap(),
    );

    stream.play().unwrap();
    Box::leak(stream);

    sender
}
