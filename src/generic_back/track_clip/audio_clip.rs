mod interleaved_audio;
pub use interleaved_audio::InterleavedAudio;

use crate::generic_back::{Arrangement, Position};
use std::{
    cmp::Ordering,
    sync::{atomic::Ordering::SeqCst, Arc},
};

pub struct AudioClip {
    pub audio: Arc<InterleavedAudio>,
    /// the start of the clip relative to the start of the arrangement
    global_start: Position,
    /// the end of the clip relative to the start of the arrangement
    global_end: Position,
    /// the start of the clip relative to the start of the sample
    clip_start: Position,
    pub arrangement: Arc<Arrangement>,
}

impl AudioClip {
    pub fn new(audio: Arc<InterleavedAudio>, arrangement: Arc<Arrangement>) -> Self {
        let samples = u32::try_from(audio.samples.len()).unwrap();

        Self {
            audio,
            global_start: Position::new(0, 0),
            global_end: Position::from_interleaved_samples(samples, &arrangement.meter),
            clip_start: Position::new(0, 0),
            arrangement,
        }
    }

    pub fn get_at_global_time(&self, global_time: u32) -> f32 {
        let start = self
            .global_start
            .in_interleaved_samples(&self.arrangement.meter);

        if !&self.arrangement.meter.playing.load(SeqCst) {
            return 0.0;
        }

        let index = global_time - start
            + self
                .clip_start
                .in_interleaved_samples(&self.arrangement.meter);

        *self
            .audio
            .samples
            .get(usize::try_from(index).unwrap())
            .unwrap_or(&0.0)
    }

    pub const fn get_global_start(&self) -> Position {
        self.global_start
    }

    pub const fn get_global_end(&self) -> Position {
        self.global_end
    }

    pub const fn get_clip_start(&self) -> Position {
        self.clip_start
    }

    pub fn trim_start_to(&mut self, clip_start: Position) {
        match self.clip_start.cmp(&clip_start) {
            Ordering::Less => {
                self.global_start += clip_start - self.clip_start;
            }
            Ordering::Equal => {}
            Ordering::Greater => {
                self.global_start -= self.clip_start - clip_start;
            }
        }
        self.clip_start = clip_start;
        assert!(self.global_start <= self.global_end);
    }

    pub fn trim_end_to(&mut self, global_end: Position) {
        self.global_end = global_end;
        assert!(self.global_start <= self.global_end);
    }

    pub fn move_to(&mut self, global_start: Position) {
        match self.global_start.cmp(&global_start) {
            Ordering::Less => {
                self.global_end += global_start - self.global_start;
            }
            Ordering::Equal => {}
            Ordering::Greater => {
                self.global_end -= self.global_start - global_start;
            }
        }
        self.global_start = global_start;
    }
}
