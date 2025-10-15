use crate::{Channel, Event, NodeAction, NodeImpl, Track, daw_ctx::State};

#[derive(Debug)]
pub enum AudioGraphNode {
	Channel(Channel),
	Track(Track),
}

const _: () = assert!(size_of::<AudioGraphNode>() <= 128);

impl NodeImpl for AudioGraphNode {
	type Event = Event;
	type State = State;

	fn process(&mut self, state: &Self::State, audio: &mut [f32], events: &mut Vec<Self::Event>) {
		match self {
			Self::Channel(node) => node.process(state, audio, events),
			Self::Track(node) => node.process(state, audio, events),
		}
	}

	fn id(&self) -> audio_graph::NodeId {
		match self {
			Self::Channel(node) => node.id(),
			Self::Track(node) => node.id(),
		}
	}

	fn delay(&self) -> usize {
		match self {
			Self::Channel(node) => node.delay(),
			Self::Track(node) => node.delay(),
		}
	}

	fn expensive(&self) -> bool {
		match self {
			Self::Channel(node) => node.expensive(),
			Self::Track(node) => node.expensive(),
		}
	}
}

impl AudioGraphNode {
	pub fn apply(&mut self, action: NodeAction) {
		match self {
			Self::Channel(node) => node.apply(action),
			Self::Track(node) => node.apply(action),
		}
	}

	pub fn reset(&mut self) {
		match self {
			Self::Channel(node) => node.reset(),
			Self::Track(node) => node.reset(),
		}
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
