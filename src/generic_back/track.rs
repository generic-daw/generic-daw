use std::sync::Arc;

use super::{
    position::{Meter, Position},
    track_clip::TrackClip,
};

pub struct Track {
    pub clips: Vec<Arc<dyn TrackClip>>,
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

    pub fn get_at_global_time(&self, global_time: u32, meter: &Meter) -> f32 {
        self.clips
            .iter()
            .map(|clip| clip.get_at_global_time(global_time, meter))
            .sum()
    }

    pub fn len(&self) -> Position {
        self.clips
            .iter()
            .map(|clip| clip.get_global_end())
            .max()
            .unwrap_or(Position::new(0, 0))
    }
}
