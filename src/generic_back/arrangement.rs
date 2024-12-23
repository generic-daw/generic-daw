use crate::generic_back::{Meter, Position, Track};
use hound::WavWriter;
use std::{
    collections::VecDeque,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering::SeqCst},
        Arc, RwLock,
    },
};

#[derive(Debug, Default)]
pub struct Arrangement {
    pub tracks: RwLock<Vec<Arc<Track>>>,
    /// information relating to the playback of the arrangement
    pub meter: Arc<Meter>,
    /// samples that are being played back live, that are not part of the arrangement
    pub live_sample_playback: RwLock<Vec<VecDeque<f32>>>,
    pub on_bar_click: RwLock<VecDeque<f32>>,
    pub off_bar_click: RwLock<VecDeque<f32>>,
    pub last_pos: RwLock<Position>,
    pub metronome: AtomicBool,
}

impl Arrangement {
    pub fn create() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn get_at_global_time(&self, global_time: u32) -> f32 {
        if self.meter.playing.load(SeqCst) && self.metronome.load(SeqCst) && global_time % 2 == 0 {
            let pos = Position::from_interleaved_samples(global_time, &self.meter);
            if pos != *self.last_pos.read().unwrap() {
                if pos.sub_quarter_note == 0 {
                    self.live_sample_playback.write().unwrap().push(
                        if pos.quarter_note % self.meter.numerator.load(SeqCst) as u16 == 0 {
                            self.on_bar_click.read().unwrap().clone()
                        } else {
                            self.off_bar_click.read().unwrap().clone()
                        },
                    );
                }
                *self.last_pos.write().unwrap() = pos;
            }
        }

        let mut sample = self
            .tracks
            .read()
            .unwrap()
            .iter()
            .map(|track| track.get_at_global_time(global_time))
            .sum::<f32>();

        if !self.meter.exporting.load(SeqCst) {
            sample += self
                .live_sample_playback
                .write()
                .unwrap()
                .iter_mut()
                .filter_map(VecDeque::pop_front)
                .sum::<f32>();

            self.live_sample_playback
                .write()
                .unwrap()
                .retain(|sample| !sample.is_empty());
        }

        sample
    }

    pub fn len(&self) -> Position {
        self.tracks
            .read()
            .unwrap()
            .iter()
            .map(|track| track.get_global_end())
            .max()
            .unwrap_or_else(Position::default)
    }

    pub fn export(&self, path: &Path) {
        self.meter.playing.store(false, SeqCst);
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

        self.meter.exporting.store(false, SeqCst);
        self.live_sample_playback.write().unwrap().clear();
    }
}
