use crate::{
    generic_back::{pan, Arrangement, AudioClip, Position, Track},
    helpers::AtomicF32,
};
use std::sync::{atomic::Ordering::SeqCst, Arc, RwLock};

pub struct AudioTrack {
    pub clips: RwLock<Vec<AudioClip>>,
    /// between 0.0 and 1.0
    pub volume: AtomicF32,
    /// between -1.0 (left) and 1.0 (right)
    pub pan: AtomicF32,
    arrangement: Arc<Arrangement>,
}

impl AudioTrack {
    pub fn create(arrangement: Arc<Arrangement>) -> Track {
        Track::Audio(Self {
            clips: RwLock::new(Vec::new()),
            volume: AtomicF32::new(1.0),
            pan: AtomicF32::new(0.0),
            arrangement,
        })
    }

    pub fn get_at_global_time(&self, global_time: u32) -> f32 {
        if !self.arrangement.meter.playing.load(SeqCst) {
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
            .map(AudioClip::get_global_end)
            .max()
            .unwrap_or(Position::new(0, 0))
    }
}
