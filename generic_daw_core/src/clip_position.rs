use crate::MusicalTime;
use atomig::Atomic;
use std::sync::atomic::Ordering::{AcqRel, Acquire, Release};

#[derive(Debug)]
pub struct ClipPosition {
    /// the start of the clip relative to the start of the arrangement
    start: Atomic<MusicalTime>,
    /// the end of the clip relative to the start of the arrangement
    end: Atomic<MusicalTime>,
    /// the start of the clip relative to the start of the underlying sample/pattern
    offset: Atomic<MusicalTime>,
}

impl Clone for ClipPosition {
    fn clone(&self) -> Self {
        Self {
            start: Atomic::new(self.start()),
            end: Atomic::new(self.end()),
            offset: Atomic::new(self.offset()),
        }
    }
}

impl ClipPosition {
    #[must_use]
    pub fn with_len(len: MusicalTime) -> Self {
        Self {
            start: Atomic::new(MusicalTime::ZERO),
            end: Atomic::new(len),
            offset: Atomic::new(MusicalTime::ZERO),
        }
    }

    #[must_use]
    pub fn start(&self) -> MusicalTime {
        self.start.load(Acquire)
    }

    #[must_use]
    pub fn end(&self) -> MusicalTime {
        self.end.load(Acquire)
    }

    #[must_use]
    pub fn offset(&self) -> MusicalTime {
        self.offset.load(Acquire)
    }

    pub fn trim_start_to(&self, mut new_start: MusicalTime) {
        let start = self.start();
        let end = self.end();
        let offset = self.offset();
        new_start = new_start.clamp(start.saturating_sub(offset), end - MusicalTime::TICK);
        let diff = start.abs_diff(new_start);
        if start < new_start {
            self.offset.fetch_add(diff, AcqRel);
        } else {
            self.offset.fetch_sub(diff, AcqRel);
        }
        self.start.store(new_start, Release);
    }

    pub fn trim_end_to(&self, mut new_end: MusicalTime) {
        let start = self.start();
        new_end = new_end.max(start + MusicalTime::TICK);
        self.end.store(new_end, Release);
    }

    pub fn move_to(&self, new_start: MusicalTime) {
        let start = self.start();
        let diff = start.abs_diff(new_start);
        if start < new_start {
            self.end.fetch_add(diff, AcqRel);
        } else {
            self.end.fetch_sub(diff, AcqRel);
        }
        self.start.store(new_start, Release);
    }
}
