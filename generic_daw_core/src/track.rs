use crate::{Channel, Clip, Event, NodeAction, NodeId, NodeImpl, NoteId, Update, audio_processor::State};

#[derive(Debug)]
pub struct Track {
	clips: Vec<Clip>,
	active_notes: Vec<ActiveNote>,
	live_events: Vec<Event>,
	channel: Channel,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ActiveNote {
	pub note_id: NoteId,
	pub key: u8,
	pub velocity: f32,
}

impl Default for Track {
	fn default() -> Self {
		Self {
			clips: Vec::new(),
			active_notes: Vec::new(),
			live_events: Vec::new(),
			channel: Channel::default(),
		}
	}
}

impl NodeImpl for Track {
	type Event = Event;
	type State = State;

	fn process(&mut self, state: &Self::State, audio: &mut [f32], events: &mut Vec<Self::Event>) {
		if self.channel.enabled() {
			self.diff_notes(state, events);
			events.append(&mut self.live_events);

			if state.transport.playing {
				let before = events.len();
				for clip in &mut self.clips {
					let (start, end) = clip.position().position().to_samples(&state.transport);
					if start < state.transport.sample + audio.len() && end >= state.transport.sample
					{
						clip.process(state, audio, events);
					}
				}
				self.apply_events(&events[before..]);
			}
		} else {
			self.live_events.clear();
		}

		self.channel.process(state, audio, events);
	}

	fn id(&self) -> NodeId {
		self.channel.id()
	}

	fn delay(&self) -> usize {
		self.channel.delay()
	}

	fn reset(&mut self) {
		self.active_notes.clear();
		self.live_events.clear();
		self.channel.reset();
	}
}

impl Track {
	pub fn apply(&mut self, action: NodeAction) {
		match action {
			NodeAction::ClipAdd(clip, idx) => self.clips.insert(idx, clip),
			NodeAction::ClipReplace(clip, idx) => self.clips[idx] = clip,
			NodeAction::ClipRemove(index) => _ = self.clips.remove(index),
			NodeAction::ClipMoveTo(index, pos) => self.clips[index].position().move_to(pos),
			NodeAction::ClipTrimStartTo(index, pos) => {
				self.clips[index].position().trim_start_to(pos);
			}
			NodeAction::ClipTrimEndTo(index, pos) => self.clips[index].position().trim_end_to(pos),
			NodeAction::TrackEvent(event) => self.live_events.push(event),
			action => self.channel.apply(action),
		}
	}

	pub fn collect_updates(&mut self, updates: &mut Vec<Update>) {
		self.channel.collect_updates(updates);
	}

	pub fn diff_notes(&mut self, state: &State, events: &mut Vec<Event>) {
		let mut notes = Vec::new();

		if state.transport.playing {
			for clip in &mut self.clips {
				let Clip::Midi(clip) = clip else {
					continue;
				};

				let (start, end) = clip.position.position().to_samples(&state.transport);
				if start < state.transport.sample && end >= state.transport.sample {
					clip.collect_notes(state, &mut notes);
				}
			}
		}

		for before in &self.active_notes {
			if notes
				.iter()
				.any(|after| after.note_id == before.note_id)
			{
				continue;
			}

			events.push(Event::Off {
				time: 0,
				key: before.key,
				velocity: before.velocity,
				note_id: before.note_id,
			});
		}

		for after in &notes {
			if self
				.active_notes
				.iter()
				.any(|before| before.note_id == after.note_id)
			{
				continue;
			}

			events.push(Event::On {
				time: 0,
				key: after.key,
				velocity: after.velocity,
				note_id: after.note_id,
			});
		}

		self.active_notes = notes;
	}

	fn apply_events(&mut self, events: &[Event]) {
		for &event in events {
			match event {
				Event::On {
					key,
					velocity,
					note_id,
					..
				} => {
					if self
						.active_notes
						.iter()
						.all(|note| note.note_id != note_id)
					{
						self.active_notes.push(ActiveNote {
							note_id,
							key,
							velocity,
						});
					}
				}
				Event::Off { note_id, .. } => {
					self.active_notes.retain(|note| note.note_id != note_id);
				}
				Event::Choke { key, note_id, .. } | Event::End { key, note_id, .. } => {
					self.active_notes.retain(|note| {
						note_id.map_or(note.key != key, |note_id| note.note_id != note_id)
					});
				}
				Event::ParamValue { .. } => {}
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{
		AutomationPatternId, MidiClip, MidiKey, MidiNote, MidiPattern, MidiPatternId, MusicalTime,
		OffsetPosition, Position, Sample, SampleId, Transport,
	};
	use std::{collections::HashMap, num::NonZero};

	fn at(samples: usize, transport: &Transport) -> MusicalTime {
		MusicalTime::from_samples(samples, transport)
	}

	fn state_with_pattern(notes: Vec<MidiNote>) -> (State, MidiPatternId) {
		let mut transport = Transport::new(NonZero::new(48_000).unwrap(), NonZero::new(2).unwrap());
		transport.playing = true;

		let pattern = MidiPattern::from_notes(notes);
		let id = pattern.id;

		(
			State {
				transport,
				samples: HashMap::<SampleId, Sample>::new(),
				midi_patterns: HashMap::from([(id, pattern)]),
				automation_patterns: HashMap::<AutomationPatternId, _>::new(),
			},
			id,
		)
	}

	fn clip(position: Position, pattern: MidiPatternId) -> Clip {
		Clip::Midi(MidiClip {
			pattern,
			position: OffsetPosition::from(position),
		})
	}

	#[test]
	fn overlapping_same_pitch_notes_keep_distinct_ids() {
		let base = Transport::new(NonZero::new(48_000).unwrap(), NonZero::new(2).unwrap());
		let note_a = MidiNote::new(MidiKey(60), 1.0, Position::new(at(0, &base), at(8, &base)));
		let note_b = MidiNote::new(MidiKey(60), 0.5, Position::new(at(4, &base), at(12, &base)));

		let (mut state, pattern) = state_with_pattern(vec![note_a, note_b]);
		let mut track = Track::default();
		track.clips.push(clip(
			Position::new(MusicalTime::ZERO, at(12, &state.transport)),
			pattern,
		));

		let mut audio = [0.0; 4];
		let mut events = Vec::new();

		track.process(&state, &mut audio, &mut events);
		assert_eq!(track.active_notes, [ActiveNote {
			note_id: note_a.id,
			key: 60,
			velocity: 1.0,
		}]);
		assert!(matches!(
			events.as_slice(),
			[Event::On {
				note_id,
				key: 60,
				velocity,
				..
			}] if *note_id == note_a.id && (*velocity - 1.0).abs() < f32::EPSILON
		));

		state.transport.sample += audio.len();
		events.clear();
		track.process(&state, &mut audio, &mut events);
		assert_eq!(track.active_notes.len(), 2);
		assert!(track.active_notes.iter().any(|note| note.note_id == note_a.id));
		assert!(track.active_notes.iter().any(|note| note.note_id == note_b.id));
		assert!(matches!(
			events.as_slice(),
			[Event::On {
				note_id,
				key: 60,
				velocity,
				..
			}] if *note_id == note_b.id && (*velocity - 0.5).abs() < f32::EPSILON
		));

		state.transport.sample += audio.len();
		events.clear();
		track.process(&state, &mut audio, &mut events);
		assert_eq!(track.active_notes, [ActiveNote {
			note_id: note_b.id,
			key: 60,
			velocity: 0.5,
		}]);
		assert!(matches!(
			events.as_slice(),
			[Event::Off {
				note_id,
				key: 60,
				velocity,
				..
			}] if *note_id == note_a.id && (*velocity - 1.0).abs() < f32::EPSILON
		));
	}

	#[test]
	fn note_end_only_clears_matching_voice() {
		let id_a = NoteId::unique();
		let id_b = NoteId::unique();

		let mut track = Track {
			clips: Vec::new(),
			active_notes: vec![
				ActiveNote {
					note_id: id_a,
					key: 60,
					velocity: 1.0,
				},
				ActiveNote {
					note_id: id_b,
					key: 60,
					velocity: 0.5,
				},
			],
			live_events: Vec::new(),
			channel: Channel::default(),
		};

		track.apply_events(&[Event::End {
			time: 0,
			key: 60,
			note_id: Some(id_a),
		}]);

		assert_eq!(track.active_notes, [ActiveNote {
			note_id: id_b,
			key: 60,
			velocity: 0.5,
		}]);
	}

	#[test]
	fn reset_clears_active_notes() {
		let base = Transport::new(NonZero::new(48_000).unwrap(), NonZero::new(2).unwrap());
		let note = MidiNote::new(MidiKey(60), 1.0, Position::new(at(0, &base), at(8, &base)));
		let (state, pattern) = state_with_pattern(vec![note]);

		let mut track = Track::default();
		track.clips.push(clip(
			Position::new(MusicalTime::ZERO, at(8, &state.transport)),
			pattern,
		));

		let mut audio = [0.0; 4];
		let mut events = Vec::new();
		track.process(&state, &mut audio, &mut events);
		assert!(!track.active_notes.is_empty());

		track.reset();
		assert!(track.active_notes.is_empty());
	}
}
