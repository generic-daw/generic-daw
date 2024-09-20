mod audio_clip;
pub use audio_clip::{AudioClip, InterleavedAudio};

mod midi_clip;
pub use midi_clip::{AtomicDirtyEvent, DirtyEvent, MidiClip, MidiNote};
