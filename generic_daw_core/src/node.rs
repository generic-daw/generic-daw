use crate::{
	Channel, Event, NodeAction, NodeImpl, Track, Update, audio_thread::State,
	channel::ThreadPoolExecutor,
};
use audio_graph::{Inject, thread_pool::Injector};

#[derive(Debug)]
pub enum Node {
	Channel(Channel),
	Track(Track),
}

impl NodeImpl for Node {
	type Event = Event;
	type State = State;
	type Inject<'a> = ThreadPoolExecutor<'a>;

	fn process(
		&mut self,
		state: &Self::State,
		audio: &mut [f32],
		events: &mut Vec<Self::Event>,
		injector: &Injector<Inject<Self>>,
	) {
		match self {
			Self::Channel(node) => node.process(state, audio, events, injector),
			Self::Track(node) => node.process(state, audio, events, injector),
		}
	}

	fn id(&self) -> audio_graph::NodeId {
		match self {
			Self::Channel(node) => node.id(),
			Self::Track(node) => node.id(),
		}
	}

	fn latency(&self) -> usize {
		match self {
			Self::Channel(node) => node.latency(),
			Self::Track(node) => node.latency(),
		}
	}

	fn reset(&mut self) {
		match self {
			Self::Channel(node) => node.reset(),
			Self::Track(node) => node.reset(),
		}
	}
}

impl Node {
	pub fn apply(&mut self, action: NodeAction, state: &State) {
		match self {
			Self::Channel(node) => node.apply(action),
			Self::Track(node) => node.apply(action, state),
		}
	}

	pub fn collect_updates(&mut self, updates: &mut Vec<Update>) {
		match self {
			Self::Channel(node) => node.collect_updates(updates),
			Self::Track(node) => node.collect_updates(updates),
		}
	}
}

impl From<Channel> for Node {
	fn from(value: Channel) -> Self {
		Self::Channel(value)
	}
}

impl From<Track> for Node {
	fn from(value: Track) -> Self {
		Self::Track(value)
	}
}
