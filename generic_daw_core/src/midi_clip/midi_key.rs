use clap_host::clack_host::events::Match;
use std::fmt::{Display, Formatter};

#[derive(Clone, Copy, Debug)]
pub enum Key {
    A = 0,
    ASharp = 1,
    B = 2,
    C = 3,
    CSharp = 4,
    D = 5,
    DSharp = 6,
    E = 7,
    F = 8,
    FSharp = 9,
    G = 10,
    GSharp = 11,
}

impl TryFrom<i8> for Key {
    type Error = ();

    fn try_from(value: i8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::A),
            1 => Ok(Self::ASharp),
            2 => Ok(Self::B),
            3 => Ok(Self::C),
            4 => Ok(Self::CSharp),
            5 => Ok(Self::D),
            6 => Ok(Self::DSharp),
            7 => Ok(Self::E),
            8 => Ok(Self::F),
            9 => Ok(Self::FSharp),
            10 => Ok(Self::G),
            11 => Ok(Self::GSharp),
            _ => Err(()),
        }
    }
}

impl Display for Key {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::A => "A",
            Self::ASharp => "A#",
            Self::B => "B",
            Self::C => "C",
            Self::CSharp => "C#",
            Self::D => "D",
            Self::DSharp => "D#",
            Self::E => "E",
            Self::F => "F",
            Self::FSharp => "F#",
            Self::G => "G",
            Self::GSharp => "G#",
        })
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct MidiKey(i8);

impl MidiKey {
    #[must_use]
    pub const fn with_key(mut self, key: Key) -> Self {
        self.0 -= self.0 % 12;
        self.0 += key as i8;
        self
    }

    #[must_use]
    pub const fn with_octave(mut self, octave: i8) -> Self {
        self.0 %= 12;
        self.0 += 12 * octave;
        self
    }
}

impl From<MidiKey> for Match<u16> {
    fn from(value: MidiKey) -> Self {
        Self::Specific((value.0 + 9) as u16)
    }
}

impl Display for MidiKey {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Key::try_from(self.0 % 12)
            .unwrap()
            .fmt(f)
            .and_then(|()| f.write_str(itoa::Buffer::new().format(self.0 / 12)))
    }
}
