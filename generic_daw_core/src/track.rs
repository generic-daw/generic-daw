use crate::{Meter, Position, TrackClip};
use audio_graph::{pan, AudioGraphNodeImpl};
use audio_track::AudioTrack;
use midi_track::MidiTrack;
use std::{
    cmp::max_by,
    sync::{atomic::Ordering::SeqCst, Arc, RwLockReadGuard},
};

pub mod audio_track;
pub mod midi_track;

#[derive(Debug)]
pub enum Track {
    Audio(AudioTrack),
    Midi(MidiTrack),
}

impl AudioGraphNodeImpl for Track {
    fn fill_buf(&self, buf_start_sample: usize, buf: &mut [f32]) {
        match self {
            Self::Audio(track) => track.fill_buf(buf_start_sample, buf),
            Self::Midi(_) => unimplemented!(),
        }

        let volume = self.get_volume();
        let [lpan, rpan] = pan(self.get_pan()).map(|s| s * volume);

        buf.iter_mut()
            .enumerate()
            .for_each(|(i, s)| *s *= if i % 2 == 0 { lpan } else { rpan });

        let max_abs_sample = match self {
            Self::Audio(track) => &track.max_abs_sample,
            Self::Midi(_) => unimplemented!(),
        };

        max_abs_sample.0.store(
            max_by(
                max_abs_sample.0.load(SeqCst),
                buf.iter()
                    .step_by(2)
                    .copied()
                    .map(f32::abs)
                    .max_by(f32::total_cmp)
                    .unwrap(),
                f32::total_cmp,
            ),
            SeqCst,
        );

        max_abs_sample.1.store(
            max_by(
                max_abs_sample.1.load(SeqCst),
                buf.iter()
                    .skip(1)
                    .step_by(2)
                    .copied()
                    .map(f32::abs)
                    .max_by(f32::total_cmp)
                    .unwrap(),
                f32::total_cmp,
            ),
            SeqCst,
        );
    }
}

impl Track {
    pub fn clips(&self) -> RwLockReadGuard<'_, Vec<Arc<TrackClip>>> {
        match self {
            Self::Audio(track) => track.clips(),
            Self::Midi(track) => track.clips(),
        }
    }

    #[must_use]
    pub fn meter(&self) -> &Meter {
        match self {
            Self::Audio(track) => &track.meter,
            Self::Midi(track) => &track.meter,
        }
    }

    #[must_use]
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

    pub fn remove_index(&self, index: usize) -> Arc<TrackClip> {
        match self {
            Self::Audio(track) => track.clips.write().unwrap().remove(index),
            Self::Midi(track) => track.clips.write().unwrap().remove(index),
        }
    }

    #[must_use]
    pub fn len(&self) -> Position {
        match self {
            Self::Audio(track) => track.len(),
            Self::Midi(track) => track.len(),
        }
    }

    #[must_use]
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

    #[must_use]
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

    #[must_use]
    pub fn get_enabled(&self) -> bool {
        match self {
            Self::Audio(track) => track.enabled.load(SeqCst),
            Self::Midi(_) => unimplemented!(),
        }
    }

    pub fn set_enabled(&self, enabled: bool) {
        match self {
            Self::Audio(track) => track.enabled.store(enabled, SeqCst),
            Self::Midi(_) => unimplemented!(),
        }
    }

    pub fn toggle_enabled(&self) {
        match self {
            Self::Audio(track) => track.enabled.fetch_not(SeqCst),
            Self::Midi(_) => unimplemented!(),
        };
    }

    pub fn get_reset_max_abs_sample(&self) -> (f32, f32) {
        match self {
            Self::Audio(track) => (
                track.max_abs_sample.0.swap(0.0, SeqCst),
                track.max_abs_sample.1.swap(0.0, SeqCst),
            ),
            Self::Midi(_) => unimplemented!(),
        }
    }
}
