use crate::{Action, Clip, Mixer, daw_ctx::State, event::Event};
use audio_graph::{NodeId, NodeImpl};

#[derive(Debug, Default)]
pub struct Track {
	pub clips: Vec<Clip>,
	pub node: Mixer,
}

impl NodeImpl for Track {
	type Event = Event;
	type State = State;

	fn process(
		&mut self,
		state: &mut Self::State,
		audio: &mut [f32],
		events: &mut Vec<Self::Event>,
	) {
		for clip in &self.clips {
			clip.process(&state.rtstate, audio, events);
		}

		self.node.process(state, audio, events);
	}

	fn id(&self) -> NodeId {
		self.node.id()
	}

	fn reset(&mut self) {
		self.node.reset();
	}

	fn delay(&self) -> usize {
		self.node.delay()
	}
}

impl Track {
	pub fn apply(&mut self, action: Action) {
		match action {
			Action::AddClip(clip) => self.clips.push(clip),
			Action::RemoveClip(index) => _ = self.clips.remove(index),
			action => self.node.apply(action),
		}
	}
}
