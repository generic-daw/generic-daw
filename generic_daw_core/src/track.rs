use crate::{Channel, Clip, Event, NodeAction, NodeId, NodeImpl, daw_ctx::State};
use generic_daw_utils::NoDebug;
use std::{cmp::Ordering, iter::repeat_n};

#[derive(Debug)]
pub struct Track {
	clips: Vec<Clip>,
	notes: NoDebug<Box<[u8; 128]>>,
	channel: Channel,
}

impl Default for Track {
	fn default() -> Self {
		Self {
			clips: Vec::new(),
			notes: Box::new([0; 128]).into(),
			channel: Channel::default(),
		}
	}
}

impl NodeImpl for Track {
	type Event = Event;
	type State = State;

	fn process(&mut self, state: &Self::State, audio: &mut [f32], events: &mut Vec<Self::Event>) {
		if self.channel.enabled() {
			self.diff_notes(state, audio, events);

			if state.rtstate.playing {
				for clip in &mut self.clips {
					let start = clip.position().start().to_samples(&state.rtstate);
					let end = clip.position().end().to_samples(&state.rtstate);

					if start < state.rtstate.sample + audio.len() && end >= state.rtstate.sample {
						clip.process(state, audio, events, &mut self.notes);
					}
				}
			}
		}

		self.channel.process(state, audio, events);
	}

	fn id(&self) -> NodeId {
		self.channel.id()
	}

	fn delay(&self) -> usize {
		self.channel.delay()
	}

	fn expensive(&self) -> bool {
		self.channel.expensive()
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
			action => self.channel.apply(action),
		}
	}

	pub fn reset(&mut self) {
		self.channel.reset();
	}

	pub fn diff_notes(&mut self, state: &State, audio: &[f32], events: &mut Vec<Event>) {
		let mut notes = [0; 128];

		if state.rtstate.playing {
			for clip in &mut self.clips {
				let start = clip.position().start().to_samples(&state.rtstate);
				let end = clip.position().end().to_samples(&state.rtstate);

				if start < state.rtstate.sample + audio.len() && end >= state.rtstate.sample {
					clip.collect_notes(state, &mut notes);
				}
			}
		}

		for (key, (before, after)) in self.notes.iter().zip(&notes).enumerate() {
			let event = match before.cmp(after) {
				Ordering::Equal => continue,
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

			events.extend(repeat_n(event, before.abs_diff(*after) as usize));
		}

		**self.notes = notes;
	}
}
