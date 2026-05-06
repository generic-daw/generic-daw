use crate::{
	Channel, Clip, ClipId, Event, MidiNote, NodeAction, NodeId, NodeImpl, Update,
	audio_thread::State, midi_clip::VoiceId, voice_alloc::VoiceAlloc,
};
use clap_host::events::Match;
use std::{collections::HashMap, num::NonZero};

#[derive(Debug)]
pub struct Track {
	clips: HashMap<ClipId, Clip>,
	voices: VoiceAlloc<VoiceId, MidiNote>,
	last_polyphony: usize,
	channel: Channel,
}

impl Default for Track {
	fn default() -> Self {
		Self {
			clips: HashMap::new(),
			voices: VoiceAlloc::new(NonZero::new(128).unwrap()),
			last_polyphony: 0,
			channel: Channel::default(),
		}
	}
}

impl NodeImpl for Track {
	type Event = Event;
	type State = State;

	fn process(&mut self, state: &Self::State, audio: &mut [f32], events: &mut Vec<Self::Event>) {
		self.voices.deactivate_all();

		if state.transport.playing {
			for clip in self.clips.values_mut() {
				clip.diff(state, audio, events, &mut self.voices);
			}
		}

		for voice in self.voices.drain_inactive() {
			events.push(Event::Off {
				time: 0,
				key: voice.info.key.0,
				velocity: voice.info.velocity,
				note_id: Match::Specific(voice.note_id),
			});
		}

		if state.transport.playing {
			for clip in self.clips.values_mut() {
				clip.process(state, audio, events, &mut self.voices);
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

	fn reset(&mut self) {
		self.channel.reset();
	}
}

impl Track {
	pub fn apply(&mut self, action: NodeAction, state: &State) {
		match action {
			NodeAction::ClipAdd(clip) => _ = self.clips.insert(clip.id(), clip),
			NodeAction::ClipRemove(id) => _ = self.clips.remove(&id),
			NodeAction::ClipMoveTo(id, pos) => self.clips.get_mut(&id).unwrap().move_to(pos),
			NodeAction::ClipTrimStartTo(id, pos) => {
				self.clips
					.get_mut(&id)
					.unwrap()
					.trim_start_to(pos, &state.transport);
			}
			NodeAction::ClipTrimEndTo(id, pos) => {
				self.clips
					.get_mut(&id)
					.unwrap()
					.trim_end_to(pos, &state.transport);
			}
			NodeAction::ClipStretchStartTo(id, pos) => {
				let Clip::Audio(clip) = self.clips.get_mut(&ClipId::Audio(id)).unwrap() else {
					unreachable!();
				};
				clip.stretch *= clip.position.stretch_start_to(pos, &state.transport);
				clip.stretch = clip
					.stretch
					.abs()
					.clamp(2f64.powi(-10), 2f64.powi(10))
					.copysign(clip.stretch);
			}
			NodeAction::ClipStretchEndTo(id, pos) => {
				let Clip::Audio(clip) = self.clips.get_mut(&ClipId::Audio(id)).unwrap() else {
					unreachable!();
				};
				clip.stretch *= clip.position.stretch_end_to(pos, &state.transport);
				clip.stretch = clip
					.stretch
					.abs()
					.clamp(2f64.powi(-10), 2f64.powi(10))
					.copysign(clip.stretch);
			}
			NodeAction::ClipReverse(id) => {
				let Clip::Audio(clip) = self.clips.get_mut(&ClipId::Audio(id)).unwrap() else {
					unreachable!();
				};
				clip.stretch *= -1.0;
				clip.position
					.reverse(state.samples[&clip.sample].len(&state.transport));
			}
			NodeAction::ClipSlipTo(id, pos) => {
				self.clips
					.get_mut(&id)
					.unwrap()
					.slip_to(pos, &state.transport);
			}
			action => self.channel.apply(action),
		}
	}

	pub fn collect_updates(&mut self, updates: &mut Vec<Update>) {
		let polyphony = self.voices.current_polyphony();
		if polyphony != self.last_polyphony {
			self.last_polyphony = polyphony;
			updates.push(Update::Polyphony(self.id(), polyphony));
		}

		self.channel.collect_updates(updates);
	}
}
