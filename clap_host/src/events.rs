pub use clack_host::events::{event_types::*, *};

#[derive(Clone, Copy, Debug)]
pub enum ClapEvent {
	NoteOn(NoteOnEvent),
	NoteOff(NoteOffEvent),
	ParamValue(ParamValueEvent),
	Midi(MidiEvent),
}

impl AsRef<UnknownEvent> for ClapEvent {
	fn as_ref(&self) -> &UnknownEvent {
		match self {
			Self::NoteOn(inner) => inner.as_ref(),
			Self::NoteOff(inner) => inner.as_ref(),
			Self::ParamValue(inner) => inner.as_ref(),
			Self::Midi(inner) => inner.as_ref(),
		}
	}
}
