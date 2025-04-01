use crate::{AudioClip, MidiClip, clip_position::ClipPosition};
use clap_host::Event;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub enum Clip {
    Audio(Arc<AudioClip>),
    Midi(Arc<MidiClip>),
}

impl Clip {
    pub fn process(&self, audio: &mut [f32], events: &mut Vec<Event>) {
        match self {
            Self::Audio(clip) => clip.process(audio, events),
            Self::Midi(clip) => clip.process(audio, events),
        }
    }

    #[must_use]
    pub fn position(&self) -> &ClipPosition {
        match self {
            Self::Audio(clip) => &clip.position,
            Self::Midi(clip) => &clip.position,
        }
    }
}

impl From<Arc<AudioClip>> for Clip {
    fn from(value: Arc<AudioClip>) -> Self {
        Self::Audio(value)
    }
}

impl From<Arc<MidiClip>> for Clip {
    fn from(value: Arc<MidiClip>) -> Self {
        Self::Midi(value)
    }
}
