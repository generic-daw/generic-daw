use crate::generic_back::{pan, Position, TrackClip};
use std::sync::{atomic::Ordering::SeqCst, Arc, Mutex, RwLock};

pub use audio_track::AudioTrack;
pub use midi_track::{AtomicDirtyEvent, DirtyEvent, MidiTrack};

mod audio_track;
mod midi_track;

#[derive(Debug)]
pub enum Track {
    Audio(AudioTrack),
    Midi(MidiTrack),
}

static TRACK_BUF: Mutex<Vec<f32>> = Mutex::new(vec![]);

impl Track {
    pub fn fill_buf(&self, buf_start_sample: u32, buf: &mut [f32]) {
        let mut track_buf = TRACK_BUF.lock().unwrap();

        for s in track_buf.iter_mut() {
            *s = 0.0;
        }

        track_buf.resize(buf.len(), 0.0);

        match self {
            Self::Audio(track) => track.fill_buf(buf_start_sample, &mut track_buf),
            Self::Midi(_) => unimplemented!(),
        }

        let volume = self.get_volume();
        let (lpan, rpan) = pan(self.get_pan());

        track_buf
            .iter()
            .map(|s| s * volume)
            .enumerate()
            .map(|(i, s)| if i % 2 == 0 { s * lpan } else { s * rpan })
            .zip(buf)
            .for_each(|(sample, buf)| *buf += sample);
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
