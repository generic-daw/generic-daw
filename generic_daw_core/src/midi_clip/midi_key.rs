use std::fmt::{Debug, Display, Formatter};

#[derive(Clone, Copy)]
pub enum Key {
	C = 0,
	CSharp = 1,
	D = 2,
	DSharp = 3,
	E = 4,
	F = 5,
	FSharp = 6,
	G = 7,
	GSharp = 8,
	A = 9,
	ASharp = 10,
	B = 11,
}

impl Key {
	#[must_use]
	pub const fn is_black(self) -> bool {
		matches!(
			self,
			Self::ASharp | Self::CSharp | Self::DSharp | Self::FSharp | Self::GSharp
		)
	}
}

impl TryFrom<u8> for Key {
	type Error = ();

	fn try_from(value: u8) -> Result<Self, Self::Error> {
		match value {
			0 => Ok(Self::C),
			1 => Ok(Self::CSharp),
			2 => Ok(Self::D),
			3 => Ok(Self::DSharp),
			4 => Ok(Self::E),
			5 => Ok(Self::F),
			6 => Ok(Self::FSharp),
			7 => Ok(Self::G),
			8 => Ok(Self::GSharp),
			9 => Ok(Self::A),
			10 => Ok(Self::ASharp),
			11 => Ok(Self::B),
			_ => Err(()),
		}
	}
}

impl Debug for Key {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		<Self as Display>::fmt(self, f)
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

#[derive(Clone, Copy, Default, Eq, Hash, PartialEq)]
pub struct MidiKey(pub u8);

impl MidiKey {
	#[must_use]
	pub const fn with_key(mut self, key: Key) -> Self {
		self.0 -= self.0 % 12;
		self.0 += key as u8;
		self
	}

	#[must_use]
	pub const fn with_octave(mut self, octave: i8) -> Self {
		self.0 %= 12;
		self.0 += 12 * (octave + 1) as u8;
		self
	}

	#[must_use]
	pub fn is_black(self) -> bool {
		Key::try_from(self.0 % 12).unwrap().is_black()
	}
}

impl Debug for MidiKey {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		<Self as Display>::fmt(self, f)
	}
}

impl Display for MidiKey {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		write!(
			f,
			"{}{}",
			Key::try_from(self.0 % 12).unwrap(),
			(self.0 as i8 / 12 - 1)
		)
	}
}
