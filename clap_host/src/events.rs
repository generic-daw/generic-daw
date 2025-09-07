pub use clack_host::events::{event_types::*, *};

#[derive(Clone, Copy, Debug)]
pub enum ClapEvent {
	NoteOn(NoteOnEvent),
	NoteOff(NoteOffEvent),
	NoteChoke(NoteChokeEvent),
	NoteEnd(NoteEndEvent),
	ParamValue(ParamValueEvent),
}

impl AsRef<UnknownEvent> for ClapEvent {
	fn as_ref(&self) -> &UnknownEvent {
		match self {
			Self::NoteOn(inner) => inner.as_ref(),
			Self::NoteOff(inner) => inner.as_ref(),
			Self::NoteChoke(inner) => inner.as_ref(),
			Self::NoteEnd(inner) => inner.as_ref(),
			Self::ParamValue(inner) => inner.as_ref(),
		}
	}
}
