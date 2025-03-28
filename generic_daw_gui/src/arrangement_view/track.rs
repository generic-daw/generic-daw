use super::track_clip::TrackClip;
use generic_daw_core::{
    AudioTrack, Meter, MidiTrack, MixerNode, Position,
    audio_graph::{AudioGraphNode, AudioGraphNodeImpl as _, NodeId},
};
use generic_daw_utils::EnumDispatcher;
use std::{
    ops::Deref as _,
    sync::{Arc, atomic::Ordering::Acquire},
};

#[derive(Clone, Debug)]
pub enum Track {
    AudioTrack(AudioTrack),
    MidiTrack(MidiTrack),
}

impl Track {
    pub fn try_add_clip(&mut self, clip: TrackClip) -> bool {
        match self {
            Self::AudioTrack(inner) => {
                let TrackClip::AudioClip(clip_inner) = clip else {
                    return false;
                };

                inner.clips.push(clip_inner);
            }
            Self::MidiTrack(inner) => {
                let TrackClip::MidiClip(clip_inner) = clip else {
                    return false;
                };

                inner.clips.push(clip_inner);
            }
        }

        true
    }

    pub fn clone_clip(&mut self, clip: usize) {
        match self {
            Self::AudioTrack(inner) => {
                let clip = inner.clips[clip].deref().clone();
                inner.clips.push(Arc::new(clip));
            }
            Self::MidiTrack(inner) => {
                let clip = inner.clips[clip].deref().clone();
                inner.clips.push(Arc::new(clip));
            }
        }
    }

    pub fn get_clip(&self, clip: usize) -> TrackClip {
        match self {
            Self::AudioTrack(inner) => inner.clips[clip].clone().into(),
            Self::MidiTrack(inner) => inner.clips[clip].clone().into(),
        }
    }

    pub fn delete_clip(&mut self, clip: usize) {
        match self {
            Self::AudioTrack(inner) => {
                inner.clips.remove(clip);
            }
            Self::MidiTrack(inner) => {
                inner.clips.remove(clip);
            }
        }
    }

    pub fn get_clip_at_global_time(&self, global_time: usize) -> Option<usize> {
        let meter = self.meter();
        let bpm = meter.bpm.load(Acquire);
        let sample_rate = meter.sample_rate;

        self.clips().enumerate().rev().find_map(|(i, clip)| {
            (clip
                .get_global_start()
                .in_interleaved_samples(bpm, sample_rate)
                <= global_time
                && global_time
                    <= clip
                        .get_global_end()
                        .in_interleaved_samples(bpm, sample_rate))
            .then_some(i)
        })
    }

    pub fn len(&self) -> Position {
        match self {
            Self::AudioTrack(inner) => inner.len(),
            Self::MidiTrack(inner) => inner.len(),
        }
    }

    pub fn clips(&self) -> impl ExactSizeIterator<Item = TrackClip> + DoubleEndedIterator {
        match self {
            Self::AudioTrack(inner) => {
                EnumDispatcher::A(inner.clips.iter().map(|clip| clip.clone().into()))
            }
            Self::MidiTrack(inner) => {
                EnumDispatcher::B(inner.clips.iter().map(|clip| clip.clone().into()))
            }
        }
    }

    pub fn node(&self) -> &Arc<MixerNode> {
        match self {
            Self::AudioTrack(inner) => &inner.node,
            Self::MidiTrack(inner) => &inner.node,
        }
    }

    pub fn meter(&self) -> &Meter {
        match self {
            Self::AudioTrack(inner) => &inner.meter,
            Self::MidiTrack(inner) => &inner.meter,
        }
    }

    pub fn id(&self) -> NodeId {
        match self {
            Self::AudioTrack(inner) => inner.id(),
            Self::MidiTrack(inner) => inner.id(),
        }
    }
}

impl From<AudioTrack> for Track {
    fn from(value: AudioTrack) -> Self {
        Self::AudioTrack(value)
    }
}

impl From<MidiTrack> for Track {
    fn from(value: MidiTrack) -> Self {
        Self::MidiTrack(value)
    }
}

impl From<Track> for AudioGraphNode {
    fn from(value: Track) -> Self {
        match value {
            Track::AudioTrack(inner) => inner.into(),
            Track::MidiTrack(inner) => inner.into(),
        }
    }
}
