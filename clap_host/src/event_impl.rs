use crate::events::{ClapEvent, UnknownEvent};
use std::fmt::Debug;

pub trait EventImpl: Debug + Sized {
	#[must_use]
	fn to_clap(self, port_index: u16, prefers_midi: bool) -> ClapEvent;
	#[must_use]
	fn try_from_unknown(value: &UnknownEvent) -> Option<Self>;
}
