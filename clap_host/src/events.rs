pub use clack_extensions::note_ports::NoteDialect;
pub use clack_host::events::{event_types::*, *};
use std::fmt::Debug;

#[derive(Clone, Copy, Debug)]
pub enum ClapEvent {
	NoteOn(NoteOnEvent),
	NoteOff(NoteOffEvent),
	ParamValue(ParamValueEvent),
	Midi(MidiEvent),
	Midi2(Midi2Event),
}

impl AsRef<UnknownEvent> for ClapEvent {
	fn as_ref(&self) -> &UnknownEvent {
		match self {
			Self::NoteOn(inner) => inner.as_ref(),
			Self::NoteOff(inner) => inner.as_ref(),
			Self::ParamValue(inner) => inner.as_ref(),
			Self::Midi(inner) => inner.as_ref(),
			Self::Midi2(inner) => inner.as_ref(),
		}
	}
}

pub trait EventImpl: Debug + Sized {
	#[must_use]
	fn to_clap(self, dialect: NoteDialect) -> ClapEvent;
	#[must_use]
	fn try_from_unknown(value: &UnknownEvent) -> Option<Self>;
}
