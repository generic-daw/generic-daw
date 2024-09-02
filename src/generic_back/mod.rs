pub mod arrangement;
pub mod clap_host;
pub mod position;
pub mod track;
pub mod track_clip;

use arrangement::Arrangement;
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    StreamConfig,
};
use std::sync::{
    atomic::{AtomicU32, Ordering},
    mpsc::Sender,
    Arc, RwLock,
};

pub enum StreamMessage {
    TogglePlay,
    Stop,
    Jump(u32),
    GetGlobalTime(u32),
}

pub fn build_output_stream(
    arrangement: Arc<RwLock<Arrangement>>,
) -> (Sender<StreamMessage>, Arc<AtomicU32>) {
    let device = cpal::default_host().default_output_device().unwrap();
    let config: &StreamConfig = &device.default_output_config().unwrap().into();
    arrangement.write().unwrap().meter.sample_rate = config.sample_rate.0;

    let (sender, receiver) = std::sync::mpsc::channel();
    let global_time = Arc::new(AtomicU32::new(0));
    let global_time_clone = global_time.clone();

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
                            .get_at_global_time(global_time.load(Ordering::SeqCst));
                        match receiver.try_recv() {
                            Ok(StreamMessage::TogglePlay) => {
                                playing ^= true;
                            }
                            Ok(StreamMessage::Stop) => {
                                playing = false;
                                global_time.store(0, Ordering::SeqCst);
                            }
                            Ok(StreamMessage::Jump(time)) => {
                                global_time.store(time, Ordering::SeqCst);
                            }
                            _ => {}
                        }
                        if playing {
                            global_time.fetch_add(1, Ordering::SeqCst);
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

    (sender, global_time_clone)
}
