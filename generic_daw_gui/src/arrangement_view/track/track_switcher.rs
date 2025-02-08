use crate::arrangement_view::track_clip::TrackClip;
use generic_daw_core::{AudioClip, MidiClip};
use std::{slice::Iter, sync::Arc};

#[derive(Debug)]
pub enum TrackSwitcher<'a> {
    AudioTrack(Iter<'a, Arc<AudioClip>>),
    MidiTrack(Iter<'a, Arc<MidiClip>>),
}

impl Iterator for TrackSwitcher<'_> {
    type Item = TrackClip;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::AudioTrack(audio) => audio.next().map(|c| c.clone().into()),
            Self::MidiTrack(midi) => midi.next().map(|c| c.clone().into()),
        }
    }
}

impl DoubleEndedIterator for TrackSwitcher<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        match self {
            Self::AudioTrack(audio) => audio.next_back().map(|c| c.clone().into()),
            Self::MidiTrack(midi) => midi.next_back().map(|c| c.clone().into()),
        }
    }
}

impl ExactSizeIterator for TrackSwitcher<'_> {
    fn len(&self) -> usize {
        match self {
            Self::AudioTrack(audio) => audio.len(),
            Self::MidiTrack(midi) => midi.len(),
        }
    }
}
