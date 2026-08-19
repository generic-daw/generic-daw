use crate::{
	Channel, Channels, Event, NodeAction, NodeImpl, Track, Transport, Update,
	audio_thread::{Inject, State},
	scratch::Scratch,
};
use audio_graph::{Injector, NodeId};

#[derive(Debug)]
pub enum Node {
	Channel(Channel),
	Track(Track),
	None,
}

impl NodeImpl for Node {
	type Event = Event;
	type State = State;
	type Scratch = Scratch;
	type Inject<'a> = Inject<'a>;

	fn process(
		&mut self,
		state: &State,
		audio: &mut [[f32; 2]],
		events: &mut Vec<Event>,
		scratch: &mut Scratch,
		injector: &Injector<Self>,
	) -> usize {
		match self {
			Self::Channel(node) => node.process(state, audio, events, scratch, injector),
			Self::Track(node) => node.process(state, audio, events, scratch, injector),
			Self::None => unreachable!(),
		}
	}

	fn id(&self) -> NodeId {
		match self {
			Self::Channel(node) => node.id(),
			Self::Track(node) => node.id(),
			Self::None => unreachable!(),
		}
	}

	fn reset(&mut self) {
		match self {
			Self::Channel(node) => node.reset(),
			Self::Track(node) => node.reset(),
			Self::None => unreachable!(),
		}
	}
}

impl Node {
	pub fn apply(&mut self, action: NodeAction, state: &State, updates: &mut Vec<Update>) {
		match self {
			Self::Channel(node) => node.apply(action),
			Self::Track(node) => node.apply(action, state, updates),
			Self::None => unreachable!(),
		}
	}

	pub fn toggle_kind(&mut self, transport: &Transport) {
		let this = std::mem::replace(self, Self::None);
		*self = match this {
			Self::Channel(node) => Self::Track(Track::from_channel(
				Channels::base(transport.input_channels),
				node,
			)),
			Self::Track(node) => Self::Channel(Track::into_channel(node)),
			Self::None => unreachable!(),
		};
	}

	pub fn collect_updates(&mut self, updates: &mut Vec<Update>) {
		match self {
			Self::Channel(node) => node.collect_updates(updates),
			Self::Track(node) => node.collect_updates(updates),
			Self::None => unreachable!(),
		}
	}

	#[must_use]
	pub fn output(&self) -> Channels {
		match self {
			Self::Channel(node) => node.output(),
			Self::Track(node) => node.output(),
			Self::None => unreachable!(),
		}
	}

	pub fn restart_all_plugins(&mut self) {
		match self {
			Self::Channel(node) => node.restart_all_plugins(),
			Self::Track(node) => node.restart_all_plugins(),
			Self::None => unreachable!(),
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
