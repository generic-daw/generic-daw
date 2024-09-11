pub mod arrangement;
pub mod meter;
pub mod position;
pub mod track;
pub mod track_clip;

use arrangement::Arrangement;
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    StreamConfig,
};
use std::sync::{atomic::Ordering::SeqCst, Arc};

pub fn build_output_stream(arrangement: Arc<Arrangement>) {
    let device = cpal::default_host().default_output_device().unwrap();
    let config: &StreamConfig = &device.default_output_config().unwrap().into();
    arrangement
        .meter
        .sample_rate
        .store(config.sample_rate.0, SeqCst);

    let stream = Box::new(
        device
            .build_output_stream(
                config,
                move |data, _| {
                    for sample in data.iter_mut() {
                        *sample = arrangement
                            .get_at_global_time(arrangement.meter.global_time.load(SeqCst));
                        if arrangement.meter.playing.load(SeqCst)
                            && !arrangement.meter.exporting.load(SeqCst)
                        {
                            arrangement.meter.global_time.fetch_add(1, SeqCst);
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
