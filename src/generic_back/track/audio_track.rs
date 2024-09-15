use super::Track;
use crate::generic_back::{
    arrangement::Arrangement, pan, position::Position, track_clip::audio_clip::AudioClip,
};
use std::sync::{atomic::Ordering::SeqCst, Arc, RwLock};

pub struct AudioTrack {
    pub clips: Vec<AudioClip>,
    /// between 0.0 and 1.0
    volume: f32,
    /// between -1.0 (left) and 1.0 (right)
    pan: f32,
    arrangement: Arc<Arrangement>,
}

impl AudioTrack {
    pub const fn create(arrangement: Arc<Arrangement>) -> Track {
        Track::Audio(RwLock::new(Self {
            clips: Vec::new(),
            volume: 1.0,
            pan: 0.0,
            arrangement,
        }))
    }

    pub(super) fn get_at_global_time(&self, global_time: u32) -> f32 {
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

    pub(super) fn get_global_end(&self) -> Position {
        self.clips
            .iter()
            .map(AudioClip::get_global_end)
            .max()
            .unwrap_or(Position::new(0, 0))
    }

    pub(super) const fn get_volume(&self) -> f32 {
        self.volume
    }

    pub(super) fn set_volume(&mut self, volume: f32) {
        self.volume = volume;
    }

    pub(super) const fn get_pan(&self) -> f32 {
        self.pan
    }

    pub(super) fn set_pan(&mut self, pan: f32) {
        self.pan = pan;
    }
}
