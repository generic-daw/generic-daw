use crate::arrangement_view::midi_pattern::MidiPattern;
use generic_daw_core::{MidiClipId, MidiPatternId, time::OffsetBeatRange};

#[derive(Clone, Copy, Debug)]
pub struct MidiClip {
	pub id: MidiClipId,
	pub pattern: MidiPatternId,
	pub position: OffsetBeatRange,
}

impl MidiClip {
	pub fn new(pattern: MidiPatternId) -> Self {
		Self {
			id: MidiClipId::unique(),
			pattern,
			position: OffsetBeatRange::default(),
		}
	}
}

#[derive(Clone, Copy, Debug)]
pub struct MidiClipRef<'a> {
	pub pattern: &'a MidiPattern,
	pub clip: &'a MidiClip,
	pub idx: (usize, usize),
}
