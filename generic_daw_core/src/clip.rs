use crate::{AudioClip, ClipPosition, Meter, MidiClip, event::Event};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub enum Clip {
    Audio(Arc<AudioClip>),
    Midi(Arc<MidiClip>),
}

impl Clip {
    pub fn process(&self, meter: &Meter, audio: &mut [f32], events: &mut Vec<Event>) {
        match self {
            Self::Audio(clip) => clip.process(meter, audio),
            Self::Midi(clip) => clip.process(meter, audio, events),
        }
    }

    #[must_use]
    pub fn position(&self) -> &ClipPosition {
        match self {
            Self::Audio(clip) => &clip.position,
            Self::Midi(clip) => &clip.position,
        }
    }

    #[must_use]
    pub fn duplicate(&self) -> Self {
        match self {
            Self::Audio(audio) => Self::Audio(Arc::new((**audio).clone())),
            Self::Midi(midi) => Self::Midi(Arc::new((**midi).clone())),
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
