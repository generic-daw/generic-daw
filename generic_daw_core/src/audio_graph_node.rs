use crate::{Action, Channel, Master, Track, daw_ctx::State, event::Event};
use audio_graph::NodeImpl;

#[derive(Debug)]
pub enum AudioGraphNode {
	Master(Master),
	Channel(Channel),
	Track(Track),
}

impl NodeImpl for AudioGraphNode {
	type Event = Event;
	type State = State;

	fn process(&mut self, state: &Self::State, audio: &mut [f32], events: &mut Vec<Self::Event>) {
		match self {
			Self::Master(node) => node.process(state, audio, events),
			Self::Channel(node) => node.process(state, audio, events),
			Self::Track(node) => node.process(state, audio, events),
		}
	}

	fn id(&self) -> audio_graph::NodeId {
		match self {
			Self::Master(node) => node.id(),
			Self::Channel(node) => node.id(),
			Self::Track(node) => node.id(),
		}
	}

	fn delay(&self) -> usize {
		match self {
			Self::Master(node) => node.delay(),
			Self::Channel(node) => node.delay(),
			Self::Track(node) => node.delay(),
		}
	}

	fn expensive(&self) -> bool {
		match self {
			Self::Master(node) => node.expensive(),
			Self::Channel(node) => node.expensive(),
			Self::Track(node) => node.expensive(),
		}
	}
}

impl AudioGraphNode {
	pub fn apply(&mut self, action: Action) {
		match self {
			Self::Master(node) => node.apply(action),
			Self::Channel(node) => node.apply(action),
			Self::Track(node) => node.apply(action),
		}
	}

	pub fn reset(&mut self) {
		match self {
			Self::Master(node) => node.reset(),
			Self::Channel(node) => node.reset(),
			Self::Track(node) => node.reset(),
		}
	}
}

impl From<Master> for AudioGraphNode {
	fn from(value: Master) -> Self {
		Self::Master(value)
	}
}

impl From<Channel> for AudioGraphNode {
	fn from(value: Channel) -> Self {
		Self::Channel(value)
	}
}

impl From<Track> for AudioGraphNode {
	fn from(value: Track) -> Self {
		Self::Track(value)
	}
}
