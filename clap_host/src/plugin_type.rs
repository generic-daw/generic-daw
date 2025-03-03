use clack_host::plugin::features::{ANALYZER, AUDIO_EFFECT, INSTRUMENT, NOTE_EFFECT};
use std::ffi::CStr;

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub enum PluginType {
    #[default]
    Unspecified,
    Instrument,
    AudioEffect,
    NoteEffect,
    Analyzer,
}

impl PluginType {
    #[must_use]
    pub fn audio_input(self) -> bool {
        matches!(self, Self::Unspecified | Self::AudioEffect | Self::Analyzer)
    }

    #[must_use]
    pub fn audio_output(self) -> bool {
        matches!(
            self,
            Self::Unspecified | Self::Instrument | Self::AudioEffect
        )
    }

    #[must_use]
    pub fn note_output(self) -> bool {
        matches!(self, Self::Unspecified | Self::NoteEffect)
    }
}

impl<'a> TryFrom<&'a CStr> for PluginType {
    type Error = ();

    fn try_from(value: &'a CStr) -> Result<Self, Self::Error> {
        if value == INSTRUMENT {
            Ok(Self::Instrument)
        } else if value == AUDIO_EFFECT {
            Ok(Self::AudioEffect)
        } else if value == NOTE_EFFECT {
            Ok(Self::NoteEffect)
        } else if value == ANALYZER {
            Ok(Self::Analyzer)
        } else {
            Err(())
        }
    }
}
