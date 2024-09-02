use super::{
    position::{Meter, Position},
    track::Track,
};
use cpal::StreamConfig;
use hound::WavWriter;
use std::{
    path::Path,
    sync::{Arc, RwLock},
};

pub struct Arrangement {
    pub tracks: Vec<Arc<RwLock<Track>>>,
    pub meter: Arc<RwLock<Meter>>,
}

impl Arrangement {
    pub const fn new(meter: Arc<RwLock<Meter>>) -> Self {
        Self {
            tracks: Vec::new(),
            meter,
        }
    }

    pub fn get_at_global_time(&self, global_time: u32, meter: &Arc<RwLock<Meter>>) -> f32 {
        self.tracks
            .iter()
            .map(|track| track.read().unwrap().get_at_global_time(global_time, meter))
            .sum::<f32>()
            .clamp(-1.0, 1.0)
    }

    fn len(&self) -> Position {
        self.tracks
            .iter()
            .map(|track| track.read().unwrap().len())
            .max()
            .unwrap()
    }

    pub fn export(&self, path: &Path, config: &StreamConfig, meter: &Arc<RwLock<Meter>>) {
        let mut writer = WavWriter::create(
            path,
            hound::WavSpec {
                channels: config.channels,
                sample_rate: config.sample_rate.0,
                bits_per_sample: 32,
                sample_format: hound::SampleFormat::Float,
            },
        )
        .unwrap();

        (0..self.len().in_interleaved_samples(meter)).for_each(|i| {
            writer
                .write_sample(self.get_at_global_time(i, meter))
                .unwrap();
        });
    }
}