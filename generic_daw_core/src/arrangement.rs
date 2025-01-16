use crate::{LiveSample, Meter, Position, Track};
use audio_graph::{AudioGraph, AudioGraphNodeImpl};
use hound::WavWriter;
use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering::SeqCst},
        Arc, OnceLock, RwLock,
    },
};

#[derive(Debug, Default)]
pub struct Arrangement {
    pub audio_graph: AudioGraph,
    /// an in-order list of all the playlist tracks in the arrangement
    pub tracks: RwLock<Vec<Arc<Track>>>,
    /// information relating to the playback of the arrangement
    pub meter: Arc<Meter>,
    /// samples that are being played back live, that are not part of the arrangement
    pub live_sample_playback: RwLock<Vec<LiveSample>>,
    /// whether the metronome is currently enabled
    pub metronome: AtomicBool,
    pub(crate) on_bar_click: OnceLock<Arc<[f32]>>,
    pub(crate) off_bar_click: OnceLock<Arc<[f32]>>,
}

impl AudioGraphNodeImpl for Arrangement {
    fn fill_buf(&self, buf_start_sample: usize, buf: &mut [f32]) {
        if self.meter.playing.load(SeqCst) && self.metronome.load(SeqCst) {
            let mut buf_start_pos =
                Position::from_interleaved_samples(buf_start_sample, &self.meter);
            let buf_end_pos =
                Position::from_interleaved_samples(buf_start_sample + buf.len(), &self.meter);

            if buf_start_pos.quarter_note() != buf_end_pos.quarter_note()
                || buf_start_pos.sub_quarter_note() == 0
            {
                buf_start_pos = buf_end_pos.floor();

                let diff = buf_start_pos.in_interleaved_samples(&self.meter) - buf_start_sample;
                let click =
                    if buf_start_pos.quarter_note() % self.meter.numerator.load(SeqCst) as u32 == 0
                    {
                        self.on_bar_click.get().unwrap().clone()
                    } else {
                        self.off_bar_click.get().unwrap().clone()
                    };

                let click = LiveSample::new(click, diff);

                self.live_sample_playback.write().unwrap().push(click);
            }
        }

        self.audio_graph.fill_buf(buf_start_sample, buf);

        if !self.meter.exporting.load(SeqCst) {
            self.live_sample_playback
                .write()
                .unwrap()
                .iter_mut()
                .for_each(|s| {
                    s.fill_buf(buf_start_sample, buf);
                });

            self.live_sample_playback
                .write()
                .unwrap()
                .retain(|sample| !sample.over());
        }
    }
}

impl Arrangement {
    #[must_use]
    pub fn create() -> Arc<Self> {
        Arc::new(Self::default())
    }

    #[must_use]
    pub fn len(&self) -> Position {
        self.tracks
            .read()
            .unwrap()
            .iter()
            .map(|track| track.len())
            .max()
            .unwrap_or_else(Position::default)
    }

    pub fn export(&self, path: &Path) {
        const CHUNK_SIZE: usize = 16;

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

        let mut buf = [0.0; CHUNK_SIZE];
        (0..self.len().in_interleaved_samples(&self.meter))
            .step_by(CHUNK_SIZE)
            .for_each(|i| {
                self.fill_buf(i, &mut buf);

                for s in buf {
                    writer.write_sample(s).unwrap();
                }
            });

        writer.finalize().unwrap();

        self.meter.exporting.store(false, SeqCst);
        self.live_sample_playback.write().unwrap().clear();
    }
}
