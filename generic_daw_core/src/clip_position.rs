use crate::Position;
use atomig::Atomic;
use std::sync::atomic::Ordering::{AcqRel, Acquire, Release};

#[derive(Debug)]
pub struct ClipPosition {
    /// the start of the clip relative to the start of the arrangement
    global_start: Atomic<Position>,
    /// the end of the clip relative to the start of the arrangement
    global_end: Atomic<Position>,
    /// the start of the clip relative to the start of the sample
    clip_start: Atomic<Position>,
}

impl Clone for ClipPosition {
    fn clone(&self) -> Self {
        Self {
            global_start: Atomic::new(self.global_start.load(Acquire)),
            global_end: Atomic::new(self.global_end.load(Acquire)),
            clip_start: Atomic::new(self.clip_start.load(Acquire)),
        }
    }
}

impl ClipPosition {
    pub fn new(global_start: Position, global_end: Position, clip_start: Position) -> Self {
        Self {
            global_start: Atomic::new(global_start),
            global_end: Atomic::new(global_end),
            clip_start: Atomic::new(clip_start),
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

    pub fn trim_start_to(&self, new_global_start: Position) {
        let global_start = self.get_global_start();
        let global_end = self.get_global_end();
        let clip_start = self.get_clip_start();
        let clamped_global_start = new_global_start.clamp(
            global_start.saturating_sub(clip_start),
            global_end - Position::SUB_QUARTER_NOTE,
        );
        let diff = global_start.abs_diff(clamped_global_start);
        if global_start < clamped_global_start {
            self.clip_start.fetch_add(diff, AcqRel);
        } else {
            self.clip_start.fetch_sub(diff, AcqRel);
        }
        self.global_start.store(clamped_global_start, Release);
    }

    pub fn trim_end_to(&self, new_global_end: Position) {
        let global_start = self.get_global_start();
        let clamped_global_end = new_global_end.max(global_start + Position::SUB_QUARTER_NOTE);
        self.global_end.store(clamped_global_end, Release);
    }

    pub fn move_to(&self, new_global_start: Position) {
        let global_start = self.get_global_start();
        let diff = global_start.abs_diff(new_global_start);
        if global_start < new_global_start {
            self.global_end.fetch_add(diff, AcqRel);
        } else {
            self.global_end.fetch_sub(diff, AcqRel);
        }
        self.global_start.store(new_global_start, Release);
    }
}
