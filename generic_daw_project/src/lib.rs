pub mod proto {
    #![expect(clippy::derive_partial_eq_without_eq)]

    use std::path::PathBuf;
    include!(concat!(env!("OUT_DIR"), "/project.rs"));

    impl project::Audio {
        #[must_use]
        pub fn path(&self) -> PathBuf {
            self.components.iter().collect()
        }
    }

    impl From<project::track::AudioClip> for project::track::Clip {
        fn from(value: project::track::AudioClip) -> Self {
            Self {
                clip: Some(project::track::clip::Clip::Audio(value)),
            }
        }
    }

    impl From<project::track::MidiClip> for project::track::Clip {
        fn from(value: project::track::MidiClip) -> Self {
            Self {
                clip: Some(project::track::clip::Clip::Midi(value)),
            }
        }
    }
}

pub mod writer;

pub use proto::project::{
    channel::ChannelIndex,
    track::{TrackIndex, audio_clip::AudioIndex, midi_clip::MidiIndex},
};
