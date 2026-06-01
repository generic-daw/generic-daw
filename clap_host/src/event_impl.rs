use crate::events::{ClapEvent, UnknownEvent};
use std::{convert::Infallible, fmt::Debug};

pub trait EventImpl: Debug + Sized {
	#[must_use]
	fn to_clap(self, port_index: u16, prefers_midi: bool) -> ClapEvent;
	#[must_use]
	fn try_from_unknown(value: &UnknownEvent) -> Option<Self>;
}

impl EventImpl for Infallible {
	fn to_clap(self, _port_index: u16, _prefers_midi: bool) -> ClapEvent {
		unreachable!()
	}

	fn try_from_unknown(_value: &UnknownEvent) -> Option<Self> {
		None
	}
}
