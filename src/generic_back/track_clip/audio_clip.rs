pub mod interleaved_audio;

use crate::generic_back::{arrangement::Arrangement, position::Position};
use interleaved_audio::InterleavedAudio;
use std::sync::{atomic::Ordering::SeqCst, Arc};

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
        let samples = audio.len();

        Self {
            audio,
            global_start: Position::new(0, 0),
            global_end: Position::from_interleaved_samples(samples, &arrangement.meter),
            clip_start: Position::new(0, 0),
            arrangement,
        }
    }

    pub(in crate::generic_back) fn get_at_global_time(&self, global_time: u32) -> f32 {
        if !&self.arrangement.meter.playing.load(SeqCst)
            || global_time
                < self
                    .global_start
                    .in_interleaved_samples(&self.arrangement.meter)
            || global_time
                > self
                    .global_end
                    .in_interleaved_samples(&self.arrangement.meter)
        {
            return 0.0;
        }
        self.audio.get_sample_at_index(
            global_time
                - self
                    .global_start
                    .in_interleaved_samples(&self.arrangement.meter)
                + self
                    .clip_start
                    .in_interleaved_samples(&self.arrangement.meter),
        )
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
        self.clip_start = clip_start;
    }

    pub fn trim_end_to(&mut self, global_end: Position) {
        self.global_end = global_end;
    }

    pub fn move_start_to(&mut self, global_start: Position) {
        match self.global_start.cmp(&global_start) {
            std::cmp::Ordering::Less => {
                self.global_end += global_start - self.global_start;
            }
            std::cmp::Ordering::Equal => {}
            std::cmp::Ordering::Greater => {
                self.global_end += self.global_start - global_start;
            }
        }
        self.global_start = global_start;
    }
}
