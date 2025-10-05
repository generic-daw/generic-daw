use crate::{Channel, Clip, Event, NodeAction, daw_ctx::State};
use audio_graph::{NodeId, NodeImpl};
use generic_daw_utils::NoDebug;
use std::{cmp::Ordering, iter::repeat_n};

#[derive(Debug)]
pub struct Track {
	clips: Vec<Clip>,
	notes: NoDebug<Box<[u8; 128]>>,
	node: Channel,
}

impl Default for Track {
	fn default() -> Self {
		Self {
			clips: Vec::new(),
			notes: Box::new([0; 128]).into(),
			node: Channel::default(),
		}
	}
}

impl NodeImpl for Track {
	type Event = Event;
	type State = State;

	fn process(&mut self, state: &Self::State, audio: &mut [f32], events: &mut Vec<Self::Event>) {
		let mut notes = [0; 128];

		for clip in &mut self.clips {
			clip.collect_notes(state, &mut notes);
		}

		self.notes
			.iter()
			.zip(notes)
			.enumerate()
			.for_each(|(key, (before, after))| {
				let event = match before.cmp(&after) {
					Ordering::Equal => return,
					Ordering::Less => Event::On {
						time: 0,
						key: key as u8,
						velocity: 1.0,
					},
					Ordering::Greater => Event::Off {
						time: 0,
						key: key as u8,
						velocity: 1.0,
					},
				};

				events.extend(repeat_n(event, before.abs_diff(after) as usize));
			});

		for clip in &mut self.clips {
			clip.process(state, audio, events, &mut notes);
		}

		**self.notes = notes;

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
