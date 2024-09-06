use super::Track;
use crate::{
    generic_back::{
        meter::Meter,
        position::Position,
        track_clip::midi_clip::{MidiClip, MidiNote},
    },
    generic_front::drawable::{Drawable, TimelinePosition, TimelineScale},
};
use iced::{widget::canvas::Frame, Theme};
use std::sync::{Arc, RwLock};

pub struct MidiTrack {
    pub clips: RwLock<Vec<Arc<MidiClip>>>,
}

impl MidiTrack {
    pub const fn new() -> Self {
        Self {
            clips: RwLock::new(Vec::new()),
        }
    }

    fn get_global_midi(&self, meter: &Meter) -> Vec<MidiNote> {
        self.clips
            .read()
            .unwrap()
            .iter()
            .flat_map(|clip| clip.get_global_midi(meter))
            .collect()
    }
}

impl Track for MidiTrack {
    fn get_at_global_time(&self, global_time: u32, meter: &Meter) -> f32 {
        self.clips
            .read()
            .unwrap()
            .iter()
            .map(|clip| clip.get_at_global_time(global_time, meter))
            .sum()
    }

    fn get_global_end(&self) -> Position {
        self.clips
            .read()
            .unwrap()
            .iter()
            .map(|clip| clip.get_global_end())
            .max()
            .unwrap_or(Position::new(0, 0))
    }
}

impl Drawable for MidiTrack {
    fn draw(
        &self,
        _frame: &mut Frame,
        _scale: TimelineScale,
        _offset: &TimelinePosition,
        _meter: &Meter,
        _theme: &Theme,
    ) {
        unimplemented!()
    }
}
