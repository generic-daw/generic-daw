use crate::{Channel, Clip, NodeAction, daw_ctx::State, event::Event};
use audio_graph::{NodeId, NodeImpl};

#[derive(Debug, Default)]
pub struct Track {
	pub clips: Vec<Clip>,
	pub node: Channel,
}

impl NodeImpl for Track {
	type Event = Event;
	type State = State;

	fn process(&mut self, state: &Self::State, audio: &mut [f32], events: &mut Vec<Self::Event>) {
		for clip in &mut self.clips {
			clip.process(state, audio, events);
		}

		self.node.process(state, audio, events);
	}

	fn id(&self) -> NodeId {
		self.node.id()
	}

	fn delay(&self) -> usize {
		self.node.delay()
	}

	fn expensive(&self) -> bool {
		self.node.expensive()
	}
}

impl Track {
	pub fn apply(&mut self, action: NodeAction) {
		match action {
			NodeAction::ClipAdd(clip) => self.clips.push(clip),
			NodeAction::ClipRemove(index) => _ = self.clips.remove(index),
			NodeAction::ClipMoveTo(index, pos) => self.clips[index].position().move_to(pos),
			NodeAction::ClipTrimStartTo(index, pos) => {
				self.clips[index].position().trim_start_to(pos);
			}
			NodeAction::ClipTrimEndTo(index, pos) => self.clips[index].position().trim_end_to(pos),
			action => self.node.apply(action),
		}
	}

	pub fn reset(&mut self) {
		self.node.reset();
	}
}
