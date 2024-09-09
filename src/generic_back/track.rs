pub mod audio_track;
pub mod midi_track;

use super::{meter::Meter, position::Position};
use crate::generic_front::drawable::Drawable;

pub trait Track: Send + Sync + Drawable {
    fn get_at_global_time(&self, global_time: usize, meter: &Meter) -> f32;
    fn get_global_end(&self) -> Position;
    fn get_volume(&self) -> f32;
    fn set_volume(&mut self, volume: f32);
}
