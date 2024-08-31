use super::{
    position::{Meter, Position},
    track::Track,
};
use cpal::StreamConfig;
use hound::WavWriter;
use std::{
    path::Path,
    sync::{Arc, Mutex},
};

pub struct Arrangement {
    pub tracks: Vec<Arc<Mutex<Track>>>,
    pub meter: Arc<Meter>,
}

impl Arrangement {
    pub const fn new(meter: Arc<Meter>) -> Self {
        Self {
            tracks: Vec::new(),
            meter,
        }
    }

    pub fn get_at_global_time(&self, global_time: u32, meter: &Arc<Meter>) -> f32 {
        self.tracks
            .iter()
            .map(|track| track.lock().unwrap().get_at_global_time(global_time, meter))
            .sum::<f32>()
            .clamp(-1.0, 1.0)
    }

    pub fn len(&self) -> Position {
        self.tracks
            .iter()
            .map(|track| track.lock().unwrap().len())
            .max()
            .unwrap()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == Position::new(0, 0)
    }

    pub fn push(&mut self, track: Track) {
        self.tracks.push(Arc::new(Mutex::new(track)));
    }

    pub fn remove(&mut self, index: usize) {
        self.tracks.remove(index);
    }

    pub fn export(&self, path: &Path, config: &StreamConfig, meter: &Arc<Meter>) {
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
