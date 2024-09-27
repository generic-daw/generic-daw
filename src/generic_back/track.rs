mod audio_track;
pub use audio_track::AudioTrack;

mod midi_track;
pub use midi_track::MidiTrack;

use crate::generic_back::{Position, TrackClip};
use std::sync::{atomic::Ordering::SeqCst, Arc, RwLock};

#[derive(Debug)]
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

    pub fn clips(&self) -> Arc<RwLock<Vec<Arc<TrackClip>>>> {
        match self {
            Self::Audio(track) => track.clips.clone(),
            Self::Midi(track) => track.clips.clone(),
        }
    }

    pub fn try_push(&self, clip: &Arc<TrackClip>) -> bool {
        match self {
            Self::Audio(track) => match **clip {
                TrackClip::Audio(_) => {
                    track.clips.write().unwrap().push(clip.clone());
                    true
                }
                TrackClip::Midi(_) => false,
            },
            Self::Midi(track) => match **clip {
                TrackClip::Midi(_) => {
                    track.clips.write().unwrap().push(clip.clone());
                    true
                }
                TrackClip::Audio(_) => false,
            },
        }
    }

    pub fn remove_clip(&self, clip: &Arc<TrackClip>) {
        match self {
            Self::Audio(track) => {
                track
                    .clips
                    .write()
                    .unwrap()
                    .retain(|c| !Arc::ptr_eq(c, clip));
            }
            Self::Midi(track) => {
                track
                    .clips
                    .write()
                    .unwrap()
                    .retain(|c| !Arc::ptr_eq(c, clip));
            }
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
