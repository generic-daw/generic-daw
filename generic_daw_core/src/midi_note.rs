use crate::Position;
use std::{
	fmt::{Debug, Display, Formatter},
	num::NonZero,
	sync::atomic::{AtomicU32, Ordering::Relaxed},
};
use utils::variants;

variants! {
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
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

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MidiKey(pub u8);

impl MidiKey {
	#[must_use]
	pub fn key(self) -> Key {
		Key::VARIANTS[usize::from(self.0) % 12]
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

static NEXT_NOTE_ID: AtomicU32 = AtomicU32::new(1);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NoteId(NonZero<u32>);

impl NoteId {
	#[must_use]
	pub fn unique() -> Self {
		Self(NEXT_NOTE_ID.fetch_add(1, Relaxed).try_into().unwrap())
	}

	#[must_use]
	pub const fn from_raw(raw: u32) -> Option<Self> {
		match NonZero::new(raw) {
			Some(raw) => Some(Self(raw)),
			None => None,
		}
	}

	#[must_use]
	pub const fn get(self) -> u32 {
		self.0.get()
	}
}

#[derive(Clone, Copy, Debug)]
pub struct MidiNote {
	pub id: NoteId,
	pub key: MidiKey,
	pub velocity: f32,
	pub position: Position,
}

impl MidiNote {
	#[must_use]
	pub fn new(key: MidiKey, velocity: f32, position: Position) -> Self {
		Self {
			id: NoteId::unique(),
			key,
			velocity,
			position,
		}
	}
}
