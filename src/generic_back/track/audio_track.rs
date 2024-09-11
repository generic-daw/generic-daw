use crate::generic_back::{
    arrangement::Arrangement, meter::pan, position::Position, track_clip::audio_clip::AudioClip,
};
use std::sync::{atomic::Ordering::SeqCst, Arc};

pub struct AudioTrack {
    pub clips: Vec<AudioClip>,
    pub volume: f32,
    /// between -1.0 (left) and 1.0 (right)
    pub pan: f32,
    arrangement: Arc<Arrangement>,
}

impl AudioTrack {
    pub const fn new(arrangement: Arc<Arrangement>) -> Self {
        Self {
            clips: Vec::new(),
            volume: 1.0,
            pan: 0.0,
            arrangement,
        }
    }

    pub fn get_at_global_time(&self, global_time: u32) -> f32 {
        if !self.arrangement.meter.playing.load(SeqCst) {
            return 0.0;
        }

        self.clips
            .iter()
            .map(|clip| clip.get_at_global_time(global_time))
            .sum::<f32>()
            * self.volume
            * pan(self.pan, global_time)
    }

    pub fn get_global_end(&self) -> Position {
        self.clips
            .iter()
            .map(AudioClip::get_global_end)
            .max()
            .unwrap_or(Position::new(0, 0))
    }
}
