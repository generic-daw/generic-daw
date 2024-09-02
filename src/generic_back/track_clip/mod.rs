use super::position::{Meter, Position};
use crate::generic_front::drawable_clip::DrawableClip;

pub mod audio_clip;
pub mod midi_clip;

pub trait TrackClip: Send + Sync + DrawableClip {
    fn get_at_global_time(&self, global_time: u32, meter: &Meter) -> f32;
    fn get_global_start(&self) -> Position;
    fn get_global_end(&self) -> Position;
    fn trim_start_to(&mut self, clip_start: Position);
    fn trim_end_to(&mut self, global_end: Position);
    fn move_start_to(&mut self, global_start: Position);
}
