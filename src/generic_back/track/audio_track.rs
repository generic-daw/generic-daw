use crate::{
    generic_back::{pan, Meter, Position, Track, TrackClip},
    helpers::AtomicF32,
};
use std::sync::{atomic::Ordering::SeqCst, Arc, RwLock};

#[derive(Debug)]
pub struct AudioTrack {
    /// these are all guaranteed to be `TrackClip::Audio`
    pub clips: Arc<RwLock<Vec<Arc<TrackClip>>>>,
    /// between 0.0 and 1.0
    pub volume: AtomicF32,
    /// between -1.0 (left) and 1.0 (right)
    pub pan: AtomicF32,
    pub meter: Arc<Meter>,
}

impl AudioTrack {
    pub fn create(meter: Arc<Meter>) -> Track {
        Track::Audio(Self {
            clips: Arc::new(RwLock::default()),
            volume: AtomicF32::new(1.0),
            pan: AtomicF32::new(0.0),
            meter,
        })
    }

    pub fn get_at_global_time(&self, global_time: u32) -> f32 {
        if !self.meter.playing.load(SeqCst) {
            return 0.0;
        }

        self.clips
            .read()
            .unwrap()
            .iter()
            .map(|clip| clip.get_at_global_time(global_time))
            .sum::<f32>()
            * self.volume.load(SeqCst)
            * pan(self.pan.load(SeqCst), global_time)
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
