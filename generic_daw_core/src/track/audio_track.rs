use crate::{Meter, Position, Track, TrackClip};
use atomig::Atomic;
use audio_graph::AudioGraphNodeImpl;
use std::sync::{
    atomic::{AtomicBool, Ordering::SeqCst},
    Arc, RwLock, RwLockReadGuard,
};

#[derive(Debug)]
pub struct AudioTrack {
    /// these are all guaranteed to be `TrackClip::Audio`
    pub(super) clips: RwLock<Vec<Arc<TrackClip>>>,
    /// 0 <= volume
    pub(super) volume: Atomic<f32>,
    /// -1 <= pan <= 1
    pub(super) pan: Atomic<f32>,
    pub(super) max_abs_sample: (Atomic<f32>, Atomic<f32>),
    pub(super) meter: Arc<Meter>,
    pub(super) enabled: AtomicBool,
}

impl AudioGraphNodeImpl for AudioTrack {
    fn fill_buf(&self, buf_start_sample: usize, buf: &mut [f32]) {
        if !self.meter.playing.load(SeqCst) || !self.enabled.load(SeqCst) {
            return;
        }

        self.clips()
            .iter()
            .for_each(|clip| clip.fill_buf(buf_start_sample, buf));
    }
}

impl AudioTrack {
    #[must_use]
    pub fn create(meter: Arc<Meter>) -> Arc<Track> {
        Arc::new(Track::Audio(Self {
            clips: RwLock::default(),
            volume: Atomic::new(1.0),
            pan: Atomic::new(0.0),
            max_abs_sample: (Atomic::new(0.0), Atomic::new(0.0)),
            meter,
            enabled: AtomicBool::new(true),
        }))
    }

    #[must_use]
    pub fn len(&self) -> Position {
        self.clips()
            .iter()
            .map(|clip| clip.get_global_end())
            .max()
            .unwrap_or_else(Position::default)
    }

    pub fn clips(&self) -> RwLockReadGuard<'_, Vec<Arc<TrackClip>>> {
        self.clips.read().unwrap()
    }
}
