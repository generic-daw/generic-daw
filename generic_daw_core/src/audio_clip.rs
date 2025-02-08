use crate::{Meter, Position};
use atomig::Atomic;
use std::sync::{
    atomic::Ordering::{AcqRel, Acquire, Release},
    Arc,
};

mod error;
mod interleaved_audio;

pub use interleaved_audio::{resample, InterleavedAudio};

#[derive(Debug)]
pub struct AudioClip {
    pub audio: Arc<InterleavedAudio>,
    /// the start of the clip relative to the start of the arrangement
    global_start: Atomic<Position>,
    /// the end of the clip relative to the start of the arrangement
    global_end: Atomic<Position>,
    /// the start of the clip relative to the start of the sample
    clip_start: Atomic<Position>,
    /// information relating to the playback of the arrangement
    pub meter: Arc<Meter>,
}

impl Clone for AudioClip {
    fn clone(&self) -> Self {
        Self {
            audio: self.audio.clone(),
            global_start: Atomic::new(self.global_start.load(Acquire)),
            global_end: Atomic::new(self.global_end.load(Acquire)),
            clip_start: Atomic::new(self.clip_start.load(Acquire)),
            meter: self.meter.clone(),
        }
    }
}

impl AudioClip {
    #[must_use]
    pub fn create(audio: Arc<InterleavedAudio>, meter: Arc<Meter>) -> Arc<Self> {
        let samples = audio.samples.len();

        Arc::new(Self {
            audio,
            global_start: Atomic::default(),
            global_end: Atomic::new(Position::from_interleaved_samples(samples, &meter)),
            clip_start: Atomic::default(),
            meter,
        })
    }

    pub fn fill_buf(&self, buf_start_sample: usize, buf: &mut [f32]) {
        let clip_start_sample = self
            .global_start
            .load(Acquire)
            .in_interleaved_samples(&self.meter);

        let diff = buf_start_sample.abs_diff(clip_start_sample);

        if buf_start_sample > clip_start_sample {
            let start_index = diff
                + self
                    .clip_start
                    .load(Acquire)
                    .in_interleaved_samples(&self.meter);

            if start_index >= self.audio.samples.len() {
                return;
            }

            self.audio.samples[start_index..]
                .iter()
                .zip(buf)
                .for_each(|(sample, buf)| {
                    *buf += sample;
                });
        } else {
            if diff >= buf.len() {
                return;
            }

            self.audio
                .samples
                .iter()
                .zip(buf[diff..].iter_mut())
                .for_each(|(sample, buf)| {
                    *buf += sample;
                });
        }
    }

    #[must_use]
    pub fn get_global_start(&self) -> Position {
        self.global_start.load(Acquire)
    }

    #[must_use]
    pub fn get_global_end(&self) -> Position {
        self.global_end.load(Acquire)
    }

    #[must_use]
    pub fn get_clip_start(&self) -> Position {
        self.clip_start.load(Acquire)
    }

    pub fn trim_start_to(&self, global_start: Position) {
        let global_start = global_start.clamp(
            self.get_global_start()
                .saturating_sub(self.get_clip_start()),
            self.get_global_end() - Position::SUB_QUARTER_NOTE,
        );
        let diff = self.get_global_start().abs_diff(global_start);
        if self.get_global_start() < global_start {
            self.clip_start.fetch_add(diff, AcqRel);
        } else {
            self.clip_start.fetch_sub(diff, AcqRel);
        }
        self.global_start.store(global_start, Release);
    }

    pub fn trim_end_to(&self, global_end: Position) {
        let global_end = global_end.max(self.get_global_start() + Position::SUB_QUARTER_NOTE);
        self.global_end.store(global_end, Release);
    }

    pub fn move_to(&self, global_start: Position) {
        let diff = self.get_global_start().abs_diff(global_start);
        if self.get_global_start() < global_start {
            self.global_end.fetch_add(diff, AcqRel);
        } else {
            self.global_end.fetch_sub(diff, AcqRel);
        }
        self.global_start.store(global_start, Release);
    }
}
