pub mod arrangement;
pub mod clap_host;
pub mod position;
pub mod track;
pub mod track_clip;

use arrangement::Arrangement;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use position::Meter;
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

#[allow(clippy::needless_pass_by_value)]
pub fn build_output_stream(
    arrangement: Arc<RwLock<Arrangement>>,
    meter: Arc<RwLock<Meter>>,
) -> (Sender<StreamMessage>, Arc<AtomicU32>) {
    let device = cpal::default_host().default_output_device().unwrap();
    let config = &device.default_output_config().unwrap().into();

    let (sender, receiver) = std::sync::mpsc::channel();
    let global_time = Arc::new(AtomicU32::new(0));
    let global_time_clone = global_time.clone();

    let mut playing = false;
    let stream = Box::new(
        device
            .build_output_stream(
                config,
                move |data, _| {
                    let global_time_r = global_time.load(Ordering::SeqCst);
                    for (i, sample) in data.iter_mut().enumerate() {
                        *sample = arrangement
                            .read()
                            .unwrap()
                            .get_at_global_time(global_time_r + i as u32, &meter);
                    }
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
                        global_time.fetch_add(u32::try_from(data.len()).unwrap(), Ordering::SeqCst);
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
