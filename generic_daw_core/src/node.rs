use crate::{
	Channel, Channels, Event, NodeAction, NodeImpl, Track, Update, audio_thread::State,
	channel::ThreadPoolExecutor,
};
use audio_graph::{Inject, NodeId, thread_pool::Injector};

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
		audio: &mut [[f32; 2]],
		events: &mut Vec<Self::Event>,
		injector: &Injector<Inject<Self>>,
	) -> usize {
		match self {
			Self::Channel(node) => node.process(state, audio, events, injector),
			Self::Track(node) => node.process(state, audio, events, injector),
		}
	}

	fn id(&self) -> NodeId {
		match self {
			Self::Channel(node) => node.id(),
			Self::Track(node) => node.id(),
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

	pub fn toggle_kind(&mut self) {
		match self {
			Self::Channel(node) => {
				*self = Self::Track(Track::from_channel(std::mem::take(node)));
			}
			Self::Track(node) => {
				*self = Self::Channel(Track::into_channel(std::mem::take(node)));
			}
		}
	}

	pub fn collect_updates(&mut self, updates: &mut Vec<Update>) {
		match self {
			Self::Channel(node) => node.collect_updates(updates),
			Self::Track(node) => node.collect_updates(updates),
		}
	}

	pub fn clear_updates(&mut self) {
		match self {
			Self::Channel(node) => node.clear_updates(),
			Self::Track(node) => node.clear_updates(),
		}
	}

	#[must_use]
	pub fn output(&self) -> Option<Channels> {
		match self {
			Self::Channel(node) => node.output(),
			Self::Track(node) => node.output(),
		}
	}

	pub fn restart_all_plugins(&mut self) {
		match self {
			Self::Channel(node) => node.restart_all_plugins(),
			Self::Track(node) => node.restart_all_plugins(),
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
