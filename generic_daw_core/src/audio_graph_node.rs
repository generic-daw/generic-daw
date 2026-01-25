use crate::{Channel, Event, NodeAction, NodeImpl, Track, Update, daw_ctx::State};

#[derive(Debug)]
pub enum AudioGraphNode {
	Channel(Channel),
	Track(Track),
}

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

	pub fn collect_updates(&mut self, updates: &mut Vec<Update>) {
		match self {
			Self::Channel(node) => node.collect_updates(updates),
			Self::Track(node) => node.collect_updates(updates),
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
