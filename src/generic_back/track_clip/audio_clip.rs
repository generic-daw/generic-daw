mod interleaved_audio;
pub use interleaved_audio::InterleavedAudio;

use crate::generic_back::{Arrangement, Position};
use std::{
    cmp::Ordering,
    sync::{atomic::Ordering::SeqCst, Arc, RwLock},
};

#[derive(Debug)]
pub struct AudioClip {
    pub audio: Arc<InterleavedAudio>,
    /// the start of the clip relative to the start of the arrangement
    global_start: RwLock<Position>,
    /// the end of the clip relative to the start of the arrangement
    global_end: RwLock<Position>,
    /// the start of the clip relative to the start of the sample
    clip_start: RwLock<Position>,
    pub arrangement: Arc<Arrangement>,
}

impl AudioClip {
    pub fn new(audio: Arc<InterleavedAudio>, arrangement: Arc<Arrangement>) -> Self {
        let samples = u32::try_from(audio.samples.len()).unwrap();

        Self {
            audio,
            global_start: RwLock::default(),
            global_end: RwLock::new(Position::from_interleaved_samples(
                samples,
                &arrangement.meter,
            )),
            clip_start: RwLock::default(),
            arrangement,
        }
    }

    pub fn get_at_global_time(&self, global_time: u32) -> f32 {
        let start = self
            .global_start
            .read()
            .unwrap()
            .in_interleaved_samples(&self.arrangement.meter);

        if !&self.arrangement.meter.playing.load(SeqCst) || global_time < start {
            return 0.0;
        }

        let index = global_time - start
            + self
                .clip_start
                .read()
                .unwrap()
                .in_interleaved_samples(&self.arrangement.meter);

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

    pub fn trim_start_to(&self, clip_start: Position) {
        let cmp = self.clip_start.read().unwrap().cmp(&clip_start);
        match cmp {
            Ordering::Less => {
                *self.global_start.write().unwrap() +=
                    clip_start - *self.clip_start.read().unwrap();
            }
            Ordering::Equal => {}
            Ordering::Greater => {
                *self.global_start.write().unwrap() -=
                    *self.clip_start.read().unwrap() - clip_start;
            }
        }
        *self.clip_start.write().unwrap() = clip_start;
        assert!(*self.global_start.read().unwrap() <= *self.global_end.read().unwrap());
    }

    pub fn trim_end_to(&self, global_end: Position) {
        *self.global_end.write().unwrap() = global_end;
        assert!(*self.global_start.read().unwrap() <= *self.global_end.read().unwrap());
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
}
