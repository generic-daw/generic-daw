use crate::Position;
use std::fmt::{Debug, Display, Formatter};
use utils::variants;

variants! {
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd, Debug)]
pub enum Key {
	C,
	CSharp,
	D,
	DSharp,
	E,
	F,
	FSharp,
	G,
	GSharp,
	A,
	ASharp,
	B,
}
}

impl Key {
	#[must_use]
	pub const fn is_black(self) -> bool {
		matches!(
			self,
			Self::CSharp | Self::DSharp | Self::FSharp | Self::GSharp | Self::ASharp
		)
	}
}

impl Display for Key {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		f.write_str(match self {
			Self::C => "C",
			Self::CSharp => "C#",
			Self::D => "D",
			Self::DSharp => "D#",
			Self::E => "E",
			Self::F => "F",
			Self::FSharp => "F#",
			Self::G => "G",
			Self::GSharp => "G#",
			Self::A => "A",
			Self::ASharp => "A#",
			Self::B => "B",
		})
	}
}

#[derive(Clone, Copy, Eq, Hash, Debug, Ord, PartialEq, PartialOrd)]
pub struct MidiKey(pub u8);

impl MidiKey {
	#[must_use]
	pub fn key(self) -> Key {
		Key::VARIANTS[self.0 as usize % 12]
	}

	#[must_use]
	pub const fn octave(self) -> i8 {
		self.0 as i8 / 12 - 1
	}
}

impl Display for MidiKey {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}{}", self.key(), self.octave())
	}
}

#[derive(Clone, Copy, Debug)]
pub struct MidiNote {
	pub key: MidiKey,
	pub velocity: f32,
	pub position: Position,
}
