use crate::generic_back::{Meter, Position, TrackClip};
use std::{
    cmp::Ordering,
    sync::{Arc, RwLock},
};

mod interleaved_audio;
pub use interleaved_audio::{resample, InterleavedAudio};

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

    pub fn get_at_global_time(&self, global_time: u32) -> f32 {
        let start = self
            .global_start
            .read()
            .unwrap()
            .in_interleaved_samples(&self.meter);
        let end = self
            .global_end
            .read()
            .unwrap()
            .in_interleaved_samples(&self.meter);

        if global_time < start || global_time > end {
            return 0.0;
        }

        let index = global_time - start
            + self
                .clip_start
                .read()
                .unwrap()
                .in_interleaved_samples(&self.meter);

        *self
            .audio
            .samples
            .get(usize::try_from(index).unwrap())
            .unwrap_or(&0.0)
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
