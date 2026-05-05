use crate::{
	Channel, Clip, Event, MidiNote, NodeAction, NodeId, NodeImpl, Update, audio_thread::State,
	midi_clip::VoiceId, voice_alloc::VoiceAlloc,
};
use clap_host::events::Match;
use std::num::NonZero;

#[derive(Debug)]
pub struct Track {
	clips: Vec<Clip>,
	voices: VoiceAlloc<VoiceId, MidiNote>,
	last_polyphony: usize,
	channel: Channel,
}

impl Default for Track {
	fn default() -> Self {
		Self {
			clips: Vec::new(),
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
			for clip in &mut self.clips {
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
			for clip in &mut self.clips {
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
			NodeAction::ClipAdd(clip, idx) => self.clips.insert(idx, clip),
			NodeAction::ClipRemove(index) => _ = self.clips.remove(index),
			NodeAction::ClipMoveTo(index, pos) => self.clips[index].move_to(pos),
			NodeAction::ClipTrimStartTo(index, pos) => {
				self.clips[index].trim_start_to(pos, &state.transport);
			}
			NodeAction::ClipTrimEndTo(index, pos) => {
				self.clips[index].trim_end_to(pos, &state.transport);
			}
			NodeAction::ClipStretchStartTo(index, pos) => {
				self.clips[index].stretch_start_to(pos, &state.transport);
			}
			NodeAction::ClipStretchEndTo(index, pos) => {
				self.clips[index].stretch_end_to(pos, &state.transport);
			}
			NodeAction::ClipReverse(index) => {
				let Clip::Audio(clip) = &mut self.clips[index] else {
					panic!();
				};
				clip.stretch *= -1.0;
				clip.position
					.reverse(state.samples[&clip.sample].len(&state.transport));
			}
			NodeAction::ClipSlipTo(index, pos) => {
				self.clips[index].slip_to(pos, &state.transport);
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
