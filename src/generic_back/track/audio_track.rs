use crate::generic_back::{Meter, Position, Track, TrackClip};
use portable_atomic::AtomicF32;
use std::sync::{atomic::Ordering::SeqCst, Arc, RwLock};

#[derive(Debug)]
pub struct AudioTrack {
    /// these are all guaranteed to be `TrackClip::Audio`
    pub clips: RwLock<Vec<Arc<TrackClip>>>,
    /// between 0.0 and 1.0
    pub volume: AtomicF32,
    /// between -1.0 (left) and 1.0 (right)
    pub pan: AtomicF32,
    pub meter: Arc<Meter>,
}

impl AudioTrack {
    pub fn create(meter: Arc<Meter>) -> Arc<Track> {
        Arc::new(Track::Audio(Self {
            clips: RwLock::default(),
            volume: AtomicF32::new(1.0),
            pan: AtomicF32::new(0.0),
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

    pub fn get_global_end(&self) -> Position {
        self.clips
            .read()
            .unwrap()
            .iter()
            .map(|clip| clip.get_global_end())
            .max()
            .unwrap_or_else(Position::default)
    }
}
