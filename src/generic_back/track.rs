pub mod audio_track;
pub mod midi_track;

use super::position::Position;
use audio_track::AudioTrack;
use midi_track::MidiTrack;
use std::sync::{Arc, RwLock};

pub enum TrackType {
    Audio(Arc<RwLock<AudioTrack>>),
    Midi(Arc<RwLock<MidiTrack>>),
}

impl TrackType {
    pub fn get_at_global_time(&self, global_time: u32) -> f32 {
        match self {
            Self::Audio(track) => track.read().unwrap().get_at_global_time(global_time),
            Self::Midi(track) => track.read().unwrap().get_at_global_time(global_time),
        }
    }

    pub fn get_global_end(&self) -> Position {
        match self {
            Self::Audio(track) => track.read().unwrap().get_global_end(),
            Self::Midi(track) => track.read().unwrap().get_global_end(),
        }
    }

    pub fn get_volume(&self) -> f32 {
        match self {
            Self::Audio(track) => track.read().unwrap().volume,
            Self::Midi(track) => track.read().unwrap().volume,
        }
    }

    pub fn set_volume(&self, volume: f32) {
        match self {
            Self::Audio(track) => track.write().unwrap().volume = volume,
            Self::Midi(track) => track.write().unwrap().volume = volume,
        }
    }
}
