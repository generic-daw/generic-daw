mod audio_track;
pub use audio_track::AudioTrack;

mod midi_track;
pub use midi_track::MidiTrack;

use crate::generic_back::{AudioClip, MidiClip, Position};
use std::sync::atomic::Ordering::SeqCst;

pub enum Track {
    Audio(AudioTrack),
    Midi(MidiTrack),
}

impl Track {
    pub fn get_at_global_time(&self, global_time: u32) -> f32 {
        match self {
            Self::Audio(track) => track.get_at_global_time(global_time),
            Self::Midi(track) => track.get_at_global_time(global_time),
        }
    }

    pub fn try_push_audio(&self, audio: AudioClip) {
        match self {
            Self::Audio(track) => track.clips.write().unwrap().push(audio),
            Self::Midi(_) => {}
        }
    }

    pub fn try_push_midi(&self, midi: MidiClip) {
        match self {
            Self::Audio(_) => {}
            Self::Midi(track) => track.clips.write().unwrap().push(midi),
        }
    }

    pub fn get_global_end(&self) -> Position {
        match self {
            Self::Audio(track) => track.get_global_end(),
            Self::Midi(track) => track.get_global_end(),
        }
    }

    pub fn get_volume(&self) -> f32 {
        match self {
            Self::Audio(track) => track.volume.load(SeqCst),
            Self::Midi(track) => track.volume.load(SeqCst),
        }
    }

    pub fn set_volume(&self, volume: f32) {
        match self {
            Self::Audio(track) => track.volume.store(volume, SeqCst),
            Self::Midi(track) => track.volume.store(volume, SeqCst),
        }
    }

    pub fn get_pan(&self) -> f32 {
        match self {
            Self::Audio(track) => track.pan.load(SeqCst),
            Self::Midi(track) => track.pan.load(SeqCst),
        }
    }

    pub fn set_pan(&self, pan: f32) {
        match self {
            Self::Audio(track) => track.pan.store(pan, SeqCst),
            Self::Midi(track) => track.pan.store(pan, SeqCst),
        }
    }
}
