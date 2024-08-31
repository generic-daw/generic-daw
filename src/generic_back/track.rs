use std::sync::Arc;

use super::{
    position::{Meter, Position},
    track_clip::TrackClip,
};

pub struct Track {
    clips: Vec<Arc<dyn TrackClip>>,
}

impl Default for Track {
    fn default() -> Self {
        Self::new()
    }
}

impl Track {
    pub fn new() -> Self {
        Self { clips: Vec::new() }
    }

    pub fn clips(&self) -> &Vec<Arc<dyn TrackClip>> {
        &self.clips
    }

    pub fn get_at_global_time(&self, global_time: u32, meter: &Arc<Meter>) -> f32 {
        self.clips
            .iter()
            .map(|clip| clip.get_at_global_time(global_time, meter.clone()))
            .sum()
    }

    pub fn len(&self) -> Position {
        self.clips
            .iter()
            .map(|clip| clip.get_global_end())
            .max()
            .unwrap_or(Position::new(0, 0))
    }

    pub fn is_empty(&self) -> bool {
        self.len() == Position::new(0, 0)
    }

    pub fn push(&mut self, audio_clip: Arc<dyn TrackClip>) {
        self.clips.push(audio_clip);
    }

    pub fn get(&self, index: usize) -> &dyn TrackClip {
        self.clips.get(index).unwrap().as_ref()
    }

    pub fn remove(&mut self, index: usize) {
        self.clips.remove(index);
    }
}
