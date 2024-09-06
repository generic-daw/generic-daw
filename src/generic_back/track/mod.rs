pub mod audio_track;
pub mod midi_track;

use super::{meter::Meter, position::Position};
use crate::generic_front::drawable::Drawable;

pub trait Track: Send + Sync + Drawable {
    fn get_at_global_time(&self, global_time: u32, meter: &Meter) -> f32;
    fn get_global_end(&self) -> Position;
}
