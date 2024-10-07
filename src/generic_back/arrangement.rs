use crate::generic_back::{Meter, PlayingBack, Position, Track};
use hound::WavWriter;
use std::{
    path::Path,
    sync::{atomic::Ordering::SeqCst, Arc, RwLock},
};

#[derive(Debug, Default)]
pub struct Arrangement {
    pub tracks: RwLock<Vec<Track>>,
    /// information relating to the playback of the arrangement
    pub meter: Arc<Meter>,
    /// samples that are being played back live, that are not part of the arrangement
    pub live_sample_playback: RwLock<Vec<PlayingBack>>,
}

impl Arrangement {
    pub fn create() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn get_at_global_time(&self, global_time: u32) -> f32 {
        self.live_sample_playback
            .write()
            .unwrap()
            .retain(|sample| !sample.over());

        self.tracks
            .read()
            .unwrap()
            .iter()
            .map(|track| track.get_at_global_time(global_time))
            .sum::<f32>()
            + self
                .live_sample_playback
                .read()
                .unwrap()
                .iter()
                .map(PlayingBack::get)
                .sum::<f32>()
    }

    pub fn len(&self) -> Position {
        self.tracks
            .read()
            .unwrap()
            .iter()
            .map(Track::get_global_end)
            .max()
            .unwrap_or_else(Position::default)
    }

    pub fn export(&self, path: &Path) {
        self.live_sample_playback.write().unwrap().clear();
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
