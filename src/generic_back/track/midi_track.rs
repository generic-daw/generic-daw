use super::Track;
use crate::{
    generic_back::{
        meter::Meter,
        position::Position,
        track_clip::{midi_clip::MidiClip, TrackClip},
    },
    generic_front::drawable::{Drawable, TimelinePosition, TimelineScale},
};
use iced::{widget::canvas::Frame, Theme};
use std::sync::{Arc, RwLock};

pub struct MidiTrack<'a> {
    pub clips: RwLock<Vec<Arc<MidiClip<'a>>>>,
}

impl<'a> Default for MidiTrack<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> MidiTrack<'a> {
    pub const fn new() -> Self {
        Self {
            clips: RwLock::new(Vec::new()),
        }
    }
}

impl<'a> Track for MidiTrack<'a> {
    type Clip = MidiClip<'a>;

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

impl<'a> Drawable for MidiTrack<'a> {
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
