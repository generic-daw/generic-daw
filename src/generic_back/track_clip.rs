mod audio_clip;
pub use audio_clip::{AudioClip, InterleavedAudio};

mod midi_clip;
pub use midi_clip::{AtomicDirtyEvent, DirtyEvent, MidiClip, MidiNote};

use crate::generic_back::Position;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub enum TrackClip {
    Audio(AudioClip),
    Midi(MidiClip),
}

impl TrackClip {
    pub fn get_name(&self) -> String {
        match self {
            Self::Audio(audio) => audio
                .audio
                .name
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
            Self::Midi(_) => "MIDI clip".to_owned(),
        }
    }

    pub fn get_at_global_time(&self, global_time: u32) -> f32 {
        match self {
            Self::Audio(audio) => audio.get_at_global_time(global_time),
            Self::Midi(_) => panic!(),
        }
    }

    pub fn get_global_start(&self) -> Position {
        match self {
            Self::Audio(audio) => audio.get_global_start(),
            Self::Midi(midi) => midi.get_global_start(),
        }
    }

    pub fn get_global_end(&self) -> Position {
        match self {
            Self::Audio(audio) => audio.get_global_end(),
            Self::Midi(midi) => midi.get_global_end(),
        }
    }

    pub fn trim_start_to(&self, clip_start: Position) {
        match self {
            Self::Audio(audio) => audio.trim_start_to(clip_start),
            Self::Midi(midi) => midi.trim_start_to(clip_start),
        }
    }

    pub fn trim_end_to(&self, global_end: Position) {
        match self {
            Self::Audio(audio) => audio.trim_end_to(global_end),
            Self::Midi(midi) => midi.trim_end_to(global_end),
        }
    }

    pub fn move_to(&self, global_start: Position) {
        match self {
            Self::Audio(audio) => audio.move_to(global_start),
            Self::Midi(midi) => midi.move_to(global_start),
        }
    }

    pub(in crate::generic_back) fn get_global_midi(&self) -> Vec<Arc<MidiNote>> {
        match self {
            Self::Audio(_) => Vec::new(),
            Self::Midi(midi) => midi.get_global_midi(),
        }
    }
}
