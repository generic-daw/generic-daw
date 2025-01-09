use crate::generic_back::{Meter, Position, TrackClip};
use std::{
    cmp::Ordering,
    sync::{Arc, RwLock},
};

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
        let samples = u32::try_from(audio.samples.len()).unwrap();

        Arc::new(TrackClip::Audio(Self {
            audio,
            global_start: RwLock::default(),
            global_end: RwLock::new(Position::from_interleaved_samples(samples, &meter)),
            clip_start: RwLock::default(),
            meter,
        }))
    }

    pub fn fill_buf(&self, buf_start_sample: u32, buf: &mut [f32]) {
        let clip_start_sample = self
            .global_start
            .read()
            .unwrap()
            .in_interleaved_samples(&self.meter);

        if buf_start_sample + u32::try_from(buf.len()).unwrap() < clip_start_sample {
            return;
        }

        let diff = buf_start_sample.abs_diff(clip_start_sample);

        if buf_start_sample > clip_start_sample {
            let start_index = diff
                + self
                    .clip_start
                    .read()
                    .unwrap()
                    .in_interleaved_samples(&self.meter);

            self.audio.samples[start_index.try_into().unwrap()..]
                .iter()
                .zip(buf.iter_mut())
                .for_each(|(sample, buf)| {
                    *buf += sample;
                });
        } else {
            self.audio
                .samples
                .iter()
                .zip(buf[diff.try_into().unwrap()..].iter_mut())
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
        let global_start = self
            .clamp(global_start)
            .min(*self.global_end.read().unwrap() - Position::MIN_STEP);
        let cmp = self.global_start.read().unwrap().cmp(&global_start);
        match cmp {
            Ordering::Less => {
                *self.clip_start.write().unwrap() +=
                    global_start - *self.global_start.read().unwrap();
            }
            Ordering::Equal => {}
            Ordering::Greater => {
                *self.clip_start.write().unwrap() -=
                    *self.global_start.read().unwrap() - global_start;
            }
        }
        *self.global_start.write().unwrap() = global_start;
    }

    pub fn trim_end_to(&self, global_end: Position) {
        let global_end = self
            .clamp(global_end)
            .max(*self.global_start.read().unwrap() + Position::MIN_STEP);
        *self.global_end.write().unwrap() = global_end;
    }

    pub fn move_to(&self, global_start: Position) {
        let cmp = self.global_start.read().unwrap().cmp(&global_start);
        match cmp {
            Ordering::Less => {
                *self.global_end.write().unwrap() +=
                    global_start - *self.global_start.read().unwrap();
            }
            Ordering::Equal => {}
            Ordering::Greater => {
                *self.global_end.write().unwrap() -=
                    *self.global_start.read().unwrap() - global_start;
            }
        }
        *self.global_start.write().unwrap() = global_start;
    }

    fn clamp(&self, position: Position) -> Position {
        position.clamp(
            self.global_start
                .read()
                .unwrap()
                .saturating_sub(*self.clip_start.read().unwrap()),
            *self.global_start.read().unwrap()
                + Position::from_interleaved_samples(
                    u32::try_from(self.audio.samples.len()).unwrap(),
                    &self.meter,
                ),
        )
    }
}
