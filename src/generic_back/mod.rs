pub mod arrangement;
pub mod position;
pub mod track;
pub mod track_clip;

use arrangement::Arrangement;
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    StreamConfig,
};
use std::sync::{atomic::Ordering::SeqCst, Arc, RwLock};

pub fn build_output_stream(arrangement: Arc<RwLock<Arrangement>>) {
    let device = cpal::default_host().default_output_device().unwrap();
    let config: &StreamConfig = &device.default_output_config().unwrap().into();
    arrangement.write().unwrap().meter.sample_rate = config.sample_rate.0;

    let global_time = arrangement.read().unwrap().meter.global_time.clone();
    let playing = arrangement.read().unwrap().meter.playing.clone();
    let exporting = arrangement.read().unwrap().meter.exporting.clone();

    let stream = Box::new(
        device
            .build_output_stream(
                config,
                move |data, _| {
                    for sample in data.iter_mut() {
                        *sample = arrangement
                            .read()
                            .unwrap()
                            .get_at_global_time(global_time.load(SeqCst));
                        if playing.load(SeqCst) && !exporting.load(SeqCst) {
                            global_time.fetch_add(1, SeqCst);
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
}
