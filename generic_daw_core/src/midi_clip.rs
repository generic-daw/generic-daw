use crate::{Meter, Position, clip_position::ClipPosition};
use std::sync::Arc;

mod midi_note;
mod midi_pattern;

pub use midi_note::MidiNote;
pub use midi_pattern::MidiPattern;

#[derive(Clone, Debug)]
pub struct MidiClip {
    pub pattern: Arc<MidiPattern>,
    /// the position of the clip relative to the start of the arrangement
    pub position: ClipPosition,
    /// information relating to the playback of the arrangement
    pub meter: Arc<Meter>,
}

impl MidiClip {
    #[must_use]
    pub fn create(pattern: Arc<MidiPattern>, meter: Arc<Meter>) -> Arc<Self> {
        let len = pattern.len();

        Arc::new(Self {
            pattern,
            position: ClipPosition::new(Position::ZERO, len, Position::ZERO),
            meter,
        })
    }
}
