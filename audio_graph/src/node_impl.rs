use crate::{EventImpl, NodeId};
use std::fmt::Debug;

pub trait NodeImpl: Debug + Send {
	type Event: EventImpl;
	type State: Send + Sync;
	fn process(&mut self, state: &Self::State, audio: &mut [f32], events: &mut Vec<Self::Event>);
	#[must_use]
	fn id(&self) -> NodeId;
	#[must_use]
	fn delay(&self) -> usize;
}
