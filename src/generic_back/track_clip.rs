use audio_clip::AudioClip;
use midi_clip::MidiClip;

pub mod audio_clip;
pub mod midi_clip;

pub enum ClipType {
    Audio(AudioClip),
    Midi(MidiClip),
}
