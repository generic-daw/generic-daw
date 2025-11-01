use crate::arrangement_view::pattern::Pattern;
use generic_daw_core::{ClipPosition, PatternId};

#[derive(Clone, Copy, Debug)]
pub struct MidiClip {
	pub pattern: PatternId,
	pub position: ClipPosition,
}

impl MidiClip {
	pub fn new(pattern: PatternId) -> Self {
		Self {
			pattern,
			position: ClipPosition::default(),
		}
	}
}

#[derive(Clone, Copy, Debug)]
pub struct MidiClipRef<'a> {
	pub pattern: &'a Pattern,
	pub clip: &'a MidiClip,
	pub idx: (usize, usize),
}
