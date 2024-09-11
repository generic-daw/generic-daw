use crate::generic_back::{
    arrangement::Arrangement, position::Position, track_clip::audio_clip::AudioClip,
};
use std::sync::{atomic::Ordering::SeqCst, Arc, RwLock};

pub struct AudioTrack {
    pub clips: RwLock<Vec<AudioClip>>,
    pub volume: f32,
    arrangement: Arc<Arrangement>,
}

impl AudioTrack {
    pub const fn new(arrangement: Arc<Arrangement>) -> Self {
        Self {
            clips: RwLock::new(Vec::new()),
            volume: 1.0,
            arrangement,
        }
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
            * self.volume
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
