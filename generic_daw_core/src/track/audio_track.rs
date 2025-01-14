use crate::{Meter, Position, Track, TrackClip};
use atomig::Atomic;
use std::sync::{atomic::Ordering::SeqCst, Arc, RwLock};

#[derive(Debug)]
pub struct AudioTrack {
    /// these are all guaranteed to be `TrackClip::Audio`
    pub(crate) clips: RwLock<Vec<Arc<TrackClip>>>,
    /// between 0.0 and 1.0
    pub(crate) volume: Atomic<f32>,
    /// between -1.0 (left) and 1.0 (right)
    pub(crate) pan: Atomic<f32>,
    pub(crate) meter: Arc<Meter>,
}

impl AudioTrack {
    #[must_use]
    pub fn create(meter: Arc<Meter>) -> Arc<Track> {
        Arc::new(Track::Audio(Self {
            clips: RwLock::default(),
            volume: Atomic::new(1.0),
            pan: Atomic::new(0.0),
            meter,
        }))
    }

    pub fn fill_buf(&self, buf_start_sample: usize, buf: &mut [f32]) {
        if !self.meter.playing.load(SeqCst) && !self.meter.exporting.load(SeqCst) {
            return;
        }

        self.clips
            .read()
            .unwrap()
            .iter()
            .for_each(|clip| clip.fill_buf(buf_start_sample, buf));
    }

    #[must_use]
    pub fn len(&self) -> Position {
        self.clips
            .read()
            .unwrap()
            .iter()
            .map(|clip| clip.get_global_end())
            .max()
            .unwrap_or_else(Position::default)
    }
}
