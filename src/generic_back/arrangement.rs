use crate::generic_back::{Meter, Position, Track};
use hound::WavWriter;
use std::{
    path::Path,
    sync::{atomic::Ordering::SeqCst, Arc, RwLock},
};

#[derive(Debug)]
pub struct Arrangement {
    pub tracks: RwLock<Vec<Track>>,
    /// information relating to the playback of the arrangement
    pub meter: Meter,
}

impl Arrangement {
    pub fn create() -> Arc<Self> {
        Arc::new(Self {
            tracks: RwLock::new(Vec::new()),
            meter: Meter::default(),
        })
    }

    pub fn get_at_global_time(&self, global_time: u32) -> f32 {
        self.tracks
            .read()
            .unwrap()
            .iter()
            .map(|track| track.get_at_global_time(global_time))
            .sum::<f32>()
            .clamp(-1.0, 1.0)
    }

    pub fn len(&self) -> Position {
        self.tracks
            .read()
            .unwrap()
            .iter()
            .map(Track::get_global_end)
            .max()
            .unwrap_or(Position::new(0, 0))
    }

    pub fn export(&self, path: &Path) {
        self.meter.playing.store(true, SeqCst);
        self.meter.exporting.store(true, SeqCst);

        let mut writer = WavWriter::create(
            path,
            hound::WavSpec {
                channels: 2,
                sample_rate: self.meter.sample_rate.load(SeqCst),
                bits_per_sample: 32,
                sample_format: hound::SampleFormat::Float,
            },
        )
        .unwrap();

        (0..self.len().in_interleaved_samples(&self.meter)).for_each(|i| {
            writer.write_sample(self.get_at_global_time(i)).unwrap();
        });

        self.meter.playing.store(false, SeqCst);
        self.meter.exporting.store(false, SeqCst);
    }
}
