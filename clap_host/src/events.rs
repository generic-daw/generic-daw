pub use clack_host::events::{event_types::*, *};

#[expect(clippy::enum_variant_names)]
#[derive(Clone, Copy, Debug)]
pub enum ClapEvent {
	NoteOnEvent(NoteOnEvent),
	NoteOffEvent(NoteOffEvent),
	NoteChokeEvent(NoteChokeEvent),
	NoteEndEvent(NoteEndEvent),
	ParamValueEvent(ParamValueEvent),
}

impl AsRef<UnknownEvent> for ClapEvent {
	fn as_ref(&self) -> &UnknownEvent {
		match self {
			Self::NoteOnEvent(inner) => inner.as_ref(),
			Self::NoteOffEvent(inner) => inner.as_ref(),
			Self::NoteChokeEvent(inner) => inner.as_ref(),
			Self::NoteEndEvent(inner) => inner.as_ref(),
			Self::ParamValueEvent(inner) => inner.as_ref(),
		}
	}
}
