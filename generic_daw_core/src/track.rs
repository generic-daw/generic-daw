use crate::{Channel, Clip, Event, NodeAction, NodeId, NodeImpl, Update, daw_ctx::State};
use std::{cmp::Ordering, iter::repeat_n};
use utils::NoDebug;

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

			if state.transport.playing {
				for clip in &mut self.clips {
					let (start, end) = clip.position().position().to_samples(&state.transport);
					if start < state.transport.sample + audio.len() && end >= state.transport.sample
					{
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
}

impl Track {
	pub fn apply(&mut self, action: NodeAction) {
		match action {
			NodeAction::ClipAdd(clip, idx) => self.clips.insert(idx, clip),
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

	pub fn collect_updates(&mut self, updates: &mut Vec<Update>) {
		self.channel.collect_updates(updates);
	}

	pub fn diff_notes(&mut self, state: &State, audio: &[f32], events: &mut Vec<Event>) {
		let mut notes = [0; 128];

		if state.transport.playing {
			for clip in &mut self.clips {
				let Clip::Midi(clip) = clip else {
					continue;
				};

				let (start, end) = clip.position.position().to_samples(&state.transport);
				if start < state.transport.sample + audio.len() && end >= state.transport.sample {
					clip.collect_notes(state, &mut notes);
				}
			}
		}

		for ((before, after), key) in self
			.notes
			.as_chunks_mut::<16>()
			.0
			.iter_mut()
			.zip(notes.as_chunks::<16>().0)
			.zip((0..).step_by(16))
		{
			if before == after {
				continue;
			}

			for ((before, after), key) in before.iter().zip(after).zip(key..) {
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

				events.extend(repeat_n(event, before.abs_diff(*after).into()));
			}

			*before = *after;
		}
	}
}
