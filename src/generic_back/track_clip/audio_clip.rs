use crate::generic_back::{Meter, Position, TrackClip};
use std::sync::{Arc, RwLock};

pub use interleaved_audio::{resample, InterleavedAudio};

mod interleaved_audio;

#[derive(Debug)]
pub struct AudioClip {
    pub audio: Arc<InterleavedAudio>,
    /// the start of the clip relative to the start of the arrangement
    global_start: RwLock<Position>,
    /// the end of the clip relative to the start of the arrangement
    global_end: RwLock<Position>,
    /// the start of the clip relative to the start of the sample
    clip_start: RwLock<Position>,
    pub meter: Arc<Meter>,
}

impl Clone for AudioClip {
    fn clone(&self) -> Self {
        Self {
            audio: self.audio.clone(),
            global_start: RwLock::new(*self.global_start.read().unwrap()),
            global_end: RwLock::new(*self.global_end.read().unwrap()),
            clip_start: RwLock::new(*self.clip_start.read().unwrap()),
            meter: self.meter.clone(),
        }
    }
}

impl AudioClip {
    pub fn create(audio: Arc<InterleavedAudio>, meter: Arc<Meter>) -> Arc<TrackClip> {
        let samples = audio.samples.len();

        Arc::new(TrackClip::Audio(Self {
            audio,
            global_start: RwLock::default(),
            global_end: RwLock::new(Position::from_interleaved_samples(samples, &meter)),
            clip_start: RwLock::default(),
            meter,
        }))
    }

    pub fn fill_buf(&self, buf_start_sample: usize, buf: &mut [f32]) {
        let clip_start_sample = self
            .global_start
            .read()
            .unwrap()
            .in_interleaved_samples(&self.meter);

        let diff = buf_start_sample.abs_diff(clip_start_sample);

        if buf_start_sample > clip_start_sample {
            let start_index = diff
                + self
                    .clip_start
                    .read()
                    .unwrap()
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
                .copied()
                .zip(buf[diff..].iter_mut())
                .for_each(|(sample, buf)| {
                    *buf += sample;
                });
        }
    }

    pub fn get_global_start(&self) -> Position {
        *self.global_start.read().unwrap()
    }

    pub fn get_global_end(&self) -> Position {
        *self.global_end.read().unwrap()
    }

    pub fn get_clip_start(&self) -> Position {
        *self.clip_start.read().unwrap()
    }

    pub fn trim_start_to(&self, global_start: Position) {
        let global_start = global_start.clamp(
            self.get_global_start()
                .saturating_sub(self.get_clip_start()),
            self.get_global_end() - Position::MIN_STEP,
        );
        let diff = self.get_global_start().abs_diff(global_start);
        if self.get_global_start() < global_start {
            *self.clip_start.write().unwrap() += diff;
        } else {
            *self.clip_start.write().unwrap() -= diff;
        }
        *self.global_start.write().unwrap() = global_start;
    }

    pub fn trim_end_to(&self, global_end: Position) {
        let global_end = global_end.max(self.get_global_start() + Position::MIN_STEP);
        *self.global_end.write().unwrap() = global_end;
    }

    pub fn move_to(&self, global_start: Position) {
        let diff = self.get_global_start().abs_diff(global_start);
        if self.get_global_start() < global_start {
            *self.global_end.write().unwrap() += diff;
        } else {
            *self.global_end.write().unwrap() -= diff;
        }
        *self.global_start.write().unwrap() = global_start;
    }
}
