use crate::arrangement_view::{midi_pattern::MidiPattern, sample::Sample};
use generic_daw_core::{AudioClip, MidiClip};

#[derive(Clone, Copy, Debug)]
pub struct AudioClipRef<'a> {
	pub sample: &'a Sample,
	pub clip: &'a AudioClip,
	pub index: (usize, usize),
}

#[derive(Clone, Copy, Debug)]
pub struct MidiClipRef<'a> {
	pub pattern: &'a MidiPattern,
	pub clip: &'a MidiClip,
	pub index: (usize, usize),
}
