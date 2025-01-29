use crate::{Meter, Position};
use audio_clip::AudioClip;
use audio_graph::AudioGraphNodeImpl;
use midi_clip::MidiClip;

pub mod audio_clip;
pub mod midi_clip;

#[derive(Clone, Debug)]
pub enum TrackClip {
    Audio(AudioClip),
    Midi(MidiClip),
}

impl AudioGraphNodeImpl for TrackClip {
    fn fill_buf(&self, buf_start_sample: usize, buf: &mut [f32]) {
        match self {
            Self::Audio(audio) => audio.fill_buf(buf_start_sample, buf),
            Self::Midi(_) => unimplemented!(),
        }
    }
}

impl TrackClip {
    #[must_use]
    pub fn get_name(&self) -> String {
        match self {
            Self::Audio(audio) => audio
                .audio
                .path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
            Self::Midi(_) => "MIDI clip".to_owned(),
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
    pub fn len(&self) -> Position {
        self.get_global_end() - self.get_global_start()
    }

    #[must_use]
    pub fn get_global_start(&self) -> Position {
        match self {
            Self::Audio(audio) => audio.get_global_start(),
            Self::Midi(midi) => midi.get_global_start(),
        }
    }

    #[must_use]
    pub fn get_global_end(&self) -> Position {
        match self {
            Self::Audio(audio) => audio.get_global_end(),
            Self::Midi(midi) => midi.get_global_end(),
        }
    }

    #[must_use]
    pub fn get_clip_start(&self) -> Position {
        match self {
            Self::Audio(audio) => audio.get_clip_start(),
            Self::Midi(midi) => midi.get_clip_start(),
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
}
