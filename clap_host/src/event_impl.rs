use crate::events::ClapEvent;
use clack_host::events::{UnknownEvent, event_types::MidiEvent};

pub trait EventImpl: Sized + Send + 'static {
	#[must_use]
	fn to_clap(&self, port_index: u16) -> ClapEvent;
	#[must_use]
	fn to_midi(&self, port_index: u16) -> MidiEvent;
	#[must_use]
	fn try_from_unknown(value: &UnknownEvent) -> Option<Self>;
}
