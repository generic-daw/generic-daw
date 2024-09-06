pub mod audio_track;
pub mod midi_track;

use super::{meter::Meter, position::Position, track_clip::TrackClip};
use crate::generic_front::drawable::Drawable;
use std::sync::Arc;

pub trait Track: Send + Sync + Drawable {
    type Clip: TrackClip
    where
        Self: Sized;
    fn get_at_global_time(&self, global_time: u32, meter: &Meter) -> f32;
    fn get_global_end(&self) -> Position;
    fn push(&self, clip: Arc<Self::Clip>)
    where
        Self: Sized;
}
