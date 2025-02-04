use crate::{resample, LiveSample, Meter, Position, Track};
use arraydeque::{ArrayDeque, Wrapping};
use audio_graph::{AudioGraphNodeImpl, NodeId};
use hound::WavWriter;
use include_data::include_f32s;
use std::{
    path::Path,
    sync::{
        atomic::Ordering::{AcqRel, Acquire, Release},
        Arc, RwLock,
    },
};

static ON_BAR_CLICK: &[f32] = include_f32s!("../../assets/on_bar_click.pcm");
static OFF_BAR_CLICK: &[f32] = include_f32s!("../../assets/off_bar_click.pcm");

#[derive(Debug)]
pub struct Arrangement {
    id: NodeId,
    /// an in-order list of all the playlist tracks in the arrangement
    pub tracks: RwLock<Vec<Arc<Track>>>,
    /// information relating to the playback of the arrangement
    pub meter: Arc<Meter>,
    /// samples that are being played back live, that are not part of the arrangement
    ///
    /// at most two should exist at once: the metronome and a potential user-controlled sample
    pub live_sample_playback: RwLock<ArrayDeque<LiveSample, 2, Wrapping>>,
    pub(crate) on_bar_click: Arc<[f32]>,
    pub(crate) off_bar_click: Arc<[f32]>,
}

impl AudioGraphNodeImpl for Arrangement {
    fn fill_buf(&self, buf_start_sample: usize, buf: &mut [f32]) {
        if self.meter.playing.load(Acquire) && self.meter.metronome.load(Acquire) {
            let buf_start_pos = Position::from_interleaved_samples(buf_start_sample, &self.meter);
            let mut buf_end_pos =
                Position::from_interleaved_samples(buf_start_sample + buf.len(), &self.meter);

            if (buf_start_pos.quarter_note() != buf_end_pos.quarter_note()
                && buf_end_pos.sub_quarter_note() != 0)
                || buf_start_pos.sub_quarter_note() == 0
            {
                buf_end_pos = buf_end_pos.floor();

                let diff = (buf_end_pos - buf_start_pos).in_interleaved_samples(&self.meter);
                let click = if buf_end_pos.quarter_note()
                    % self.meter.numerator.load(Acquire) as u32
                    == 0
                {
                    self.on_bar_click.clone()
                } else {
                    self.off_bar_click.clone()
                };

                let click = LiveSample::new(click, diff);

                self.live_sample_playback.write().unwrap().push_back(click);
            }
        }

        self.live_sample_playback
            .read()
            .unwrap()
            .iter()
            .for_each(|s| {
                s.fill_buf(buf_start_sample, buf);
            });
    }

    fn id(&self) -> NodeId {
        self.id
    }
}

impl Arrangement {
    pub(crate) fn new(sample_rate: u32) -> Self {
        Self {
            id: NodeId::unique(),
            tracks: RwLock::default(),
            meter: Arc::new(Meter::new(sample_rate)),
            live_sample_playback: RwLock::default(),
            on_bar_click: resample(44100, sample_rate, ON_BAR_CLICK.into())
                .unwrap()
                .into(),
            off_bar_click: resample(44100, sample_rate, OFF_BAR_CLICK.into())
                .unwrap()
                .into(),
        }
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
        const CHUNK_SIZE: usize = 64;

        let live_sample_playback = std::mem::take(&mut *self.live_sample_playback.write().unwrap());
        let playing = self.meter.playing.swap(true, AcqRel);
        let metronome = self.meter.metronome.swap(false, AcqRel);

        let mut writer = WavWriter::create(
            path,
            hound::WavSpec {
                channels: 2,
                sample_rate: self.meter.sample_rate.load(Acquire),
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

        *self.live_sample_playback.write().unwrap() = live_sample_playback;
        self.meter.playing.store(playing, Release);
        self.meter.metronome.store(metronome, Release);
    }
}
