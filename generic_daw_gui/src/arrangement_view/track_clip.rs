use generic_daw_core::{AudioClip, MidiClip, Position};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub enum TrackClip {
    AudioClip(Arc<AudioClip>),
    MidiClip(Arc<MidiClip>),
}

impl TrackClip {
    pub fn get_global_start(&self) -> Position {
        match self {
            Self::AudioClip(inner) => inner.position.get_global_start(),
            Self::MidiClip(inner) => inner.position.get_global_start(),
        }
    }

    pub fn get_global_end(&self) -> Position {
        match self {
            Self::AudioClip(inner) => inner.position.get_global_end(),
            Self::MidiClip(inner) => inner.position.get_global_end(),
        }
    }

    pub fn get_clip_start(&self) -> Position {
        match self {
            Self::AudioClip(inner) => inner.position.get_clip_start(),
            Self::MidiClip(inner) => inner.position.get_clip_start(),
        }
    }

    pub fn trim_start_to(&self, global_start: Position) {
        match self {
            Self::AudioClip(inner) => inner.position.trim_start_to(global_start),
            Self::MidiClip(inner) => inner.position.trim_start_to(global_start),
        }
    }

    pub fn trim_end_to(&self, global_start: Position) {
        match self {
            Self::AudioClip(inner) => inner.position.trim_end_to(global_start),
            Self::MidiClip(inner) => inner.position.trim_end_to(global_start),
        }
    }

    pub fn move_to(&self, global_start: Position) {
        match self {
            Self::AudioClip(inner) => inner.position.move_to(global_start),
            Self::MidiClip(inner) => inner.position.move_to(global_start),
        }
    }
}

impl From<Arc<AudioClip>> for TrackClip {
    fn from(value: Arc<AudioClip>) -> Self {
        Self::AudioClip(value)
    }
}

impl From<Arc<MidiClip>> for TrackClip {
    fn from(value: Arc<MidiClip>) -> Self {
        Self::MidiClip(value)
    }
}

impl TryFrom<TrackClip> for Arc<AudioClip> {
    type Error = ();

    fn try_from(value: TrackClip) -> Result<Self, ()> {
        let TrackClip::AudioClip(inner) = value else {
            return Err(());
        };

        Ok(inner)
    }
}

impl TryFrom<TrackClip> for Arc<MidiClip> {
    type Error = ();

    fn try_from(value: TrackClip) -> Result<Self, ()> {
        let TrackClip::MidiClip(inner) = value else {
            return Err(());
        };

        Ok(inner)
    }
}
