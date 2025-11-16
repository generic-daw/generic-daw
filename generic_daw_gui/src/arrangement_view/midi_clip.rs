use crate::arrangement_view::midi_pattern::MidiPattern;
use generic_daw_core::{ClipPosition, MidiPatternId};

#[derive(Clone, Copy, Debug)]
pub struct MidiClip {
	pub pattern: MidiPatternId,
	pub position: ClipPosition,
}

impl MidiClip {
	pub fn new(pattern: MidiPatternId) -> Self {
		Self {
			pattern,
			position: ClipPosition::default(),
		}
	}
}

#[derive(Clone, Copy, Debug)]
pub struct MidiClipRef<'a> {
	pub pattern: &'a MidiPattern,
	pub clip: &'a MidiClip,
	pub idx: (usize, usize),
}
