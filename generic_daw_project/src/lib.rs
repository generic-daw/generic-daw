pub mod proto {
    #![expect(clippy::derive_partial_eq_without_eq)]

    use std::{ffi::CStr, path::PathBuf};
    include!(concat!(env!("OUT_DIR"), "/project.rs"));

    impl project::Audio {
        #[must_use]
        pub fn path(&self) -> PathBuf {
            self.components.iter().collect()
        }
    }

    impl project::channel::Plugin {
        #[must_use]
        pub fn id(&self) -> &CStr {
            CStr::from_bytes_with_nul(&self.id).unwrap()
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

pub mod reader;
pub mod writer;
