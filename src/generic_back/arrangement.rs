use super::{meter::Meter, position::Position, track::Track};
use hound::WavWriter;
use std::{
    path::Path,
    sync::{atomic::Ordering::SeqCst, Arc},
};

pub struct Arrangement {
    pub tracks: Vec<Arc<dyn Track>>,
    pub meter: Meter,
}

impl Arrangement {
    pub const fn new(meter: Meter) -> Self {
        Self {
            tracks: Vec::new(),
            meter,
        }
    }

    pub fn get_at_global_time(&self, global_time: u32) -> f32 {
        self.tracks
            .iter()
            .map(|track| track.get_at_global_time(global_time, &self.meter))
            .sum::<f32>()
            .clamp(-1.0, 1.0)
    }

    pub fn len(&self) -> Position {
        self.tracks
            .iter()
            .map(|track| track.get_global_end())
            .max()
            .unwrap_or(Position::new(0, 0))
    }

    pub fn export(&self, path: &Path, meter: &Meter) {
        self.meter.playing.store(true, SeqCst);
        self.meter.exporting.store(true, SeqCst);

        let mut writer = WavWriter::create(
            path,
            hound::WavSpec {
                channels: 2,
                sample_rate: meter.sample_rate,
                bits_per_sample: 32,
                sample_format: hound::SampleFormat::Float,
            },
        )
        .unwrap();

        (0..self.len().in_interleaved_samples(meter)).for_each(|i| {
            writer.write_sample(self.get_at_global_time(i)).unwrap();
        });

        self.meter.playing.store(false, SeqCst);
        self.meter.exporting.store(false, SeqCst);
    }
}
