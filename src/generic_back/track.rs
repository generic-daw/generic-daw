pub mod audio_track;
pub mod midi_track;

use super::{
    position::Position,
    track_clip::{audio_clip::AudioClip, midi_clip::MidiClip},
};
use audio_track::AudioTrack;
use midi_track::MidiTrack;

pub enum Track {
    Audio(AudioTrack),
    Midi(MidiTrack),
}

impl Track {
    pub(super) fn get_at_global_time(&self, global_time: u32) -> f32 {
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
            Self::Audio(track) => track.get_volume(),
            Self::Midi(track) => track.get_volume(),
        }
    }

    pub fn set_volume(&self, volume: f32) {
        match self {
            Self::Audio(track) => track.set_volume(volume),
            Self::Midi(track) => track.set_volume(volume),
        }
    }

    pub fn get_pan(&self) -> f32 {
        match self {
            Self::Audio(track) => track.get_pan(),
            Self::Midi(track) => track.get_pan(),
        }
    }

    pub fn set_pan(&self, pan: f32) {
        match self {
            Self::Audio(track) => track.set_pan(pan),
            Self::Midi(track) => track.set_pan(pan),
        }
    }
}
