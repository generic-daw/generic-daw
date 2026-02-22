use crate::events::{ClapEvent, MidiEvent, UnknownEvent};
use std::fmt::Debug;

pub trait EventImpl: Debug + Sized {
	#[must_use]
	fn to_clap(&self, port_index: u16) -> ClapEvent;
	#[must_use]
	fn to_midi(&self, port_index: u16) -> MidiEvent;
	#[must_use]
	fn try_from_unknown(value: &UnknownEvent) -> Option<Self>;
}
