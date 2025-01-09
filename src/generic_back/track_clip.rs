use crate::generic_back::Position;

pub use audio_clip::{resample, AudioClip, InterleavedAudio};
pub use midi_clip::{MidiClip, MidiNote};

mod audio_clip;
mod midi_clip;

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

    pub fn fill_buf(&self, buf_start_sample: u32, buf: &mut [f32]) {
        match self {
            Self::Audio(audio) => audio.fill_buf(buf_start_sample, buf),
            Self::Midi(_) => unimplemented!(),
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
}
