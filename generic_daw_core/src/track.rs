use crate::{
	Channel, Clip, ClipId, Event, MidiNote, Node, NodeAction, NodeId, Update, audio_thread::State,
	midi_clip::VoiceId, voice_alloc::VoiceAlloc,
};
use audio_graph::{Inject, thread_pool::Injector};
use clap_host::events::Match;
use std::{collections::HashMap, num::NonZero};

#[derive(Debug)]
pub struct Track {
	clips: HashMap<ClipId, Clip>,
	voice_alloc: VoiceAlloc<VoiceId, MidiNote>,
	last_polyphony: usize,
	channel: Channel,
}

impl Default for Track {
	fn default() -> Self {
		Self {
			clips: HashMap::new(),
			voice_alloc: VoiceAlloc::new(NonZero::new(128).unwrap()),
			last_polyphony: 0,
			channel: Channel::default(),
		}
	}
}

impl Track {
	pub fn process(
		&mut self,
		state: &State,
		audio: &mut [[f32; 2]],
		events: &mut Vec<Event>,
		injector: &Injector<Inject<Node>>,
	) -> usize {
		self.voice_alloc.deactivate_all();

		if state.transport.playing {
			for clip in self.clips.values_mut() {
				clip.diff(state, audio, events, &mut self.voice_alloc);
			}
		}

		for voice in self.voice_alloc.drain_inactive() {
			events.push(Event::Off {
				time: 0,
				key: voice.info.key.0,
				velocity: voice.info.velocity,
				note_id: Match::Specific(voice.note_id),
			});
		}

		if state.transport.playing {
			for clip in self.clips.values_mut() {
				clip.process(state, audio, events, &mut self.voice_alloc);
			}
		}

		let latency = self.channel.process(state, audio, events, injector);

		if state.transport.solo.is_none_or(|solo| solo == self.id()) {
			latency
		} else {
			audio.fill([0.0; 2]);
			events.clear();
			0
		}
	}

	#[must_use]
	pub fn id(&self) -> NodeId {
		self.channel.id()
	}

	pub fn reset(&mut self) {
		self.channel.reset();
	}

	pub fn apply(&mut self, action: NodeAction, state: &State) {
		match action {
			NodeAction::ClipAdd(clip) => _ = self.clips.insert(clip.id(), *clip),
			NodeAction::ClipRemove(id) => _ = self.clips.remove(&id),
			NodeAction::ClipMoveTo(id, pos) => self.clips.get_mut(&id).unwrap().move_to(pos),
			NodeAction::ClipTrimStartTo(id, pos) => {
				let clip = self.clips.get_mut(&id).unwrap();
				clip.trim_start_to(pos, &state.transport);
				if let Clip::Audio(audio) = clip {
					audio.fade_start.len = audio.fade_start.len.min(audio.position.len());
					audio.fade_end.len = audio
						.fade_end
						.len
						.min(audio.position.len() - audio.fade_start.len);
				}
			}
			NodeAction::ClipTrimEndTo(id, pos) => {
				let clip = self.clips.get_mut(&id).unwrap();
				clip.trim_end_to(pos, &state.transport);
				if let Clip::Audio(audio) = clip {
					audio.fade_end.len = audio.fade_end.len.min(audio.position.len());
					audio.fade_start.len = audio
						.fade_start
						.len
						.min(audio.position.len() - audio.fade_end.len);
				}
			}
			NodeAction::ClipVolumeChanged(id, volume) => {
				let Clip::Audio(clip) = self.clips.get_mut(&id).unwrap() else {
					panic!();
				};
				clip.volume = volume;
			}
			NodeAction::ClipFadeStartLen(id, len) => {
				let Clip::Audio(clip) = self.clips.get_mut(&id).unwrap() else {
					panic!();
				};
				clip.fade_start.len = len;
			}
			NodeAction::ClipFadeStartP(id, p) => {
				let Clip::Audio(clip) = self.clips.get_mut(&id).unwrap() else {
					panic!();
				};
				clip.fade_start.p = p;
			}
			NodeAction::ClipFadeStartToggleSymmetric(id) => {
				let Clip::Audio(clip) = self.clips.get_mut(&id).unwrap() else {
					panic!();
				};
				clip.fade_start.symmetric ^= true;
			}
			NodeAction::ClipFadeEndLen(id, len) => {
				let Clip::Audio(clip) = self.clips.get_mut(&id).unwrap() else {
					panic!();
				};
				clip.fade_end.len = len;
			}
			NodeAction::ClipFadeEndP(id, p) => {
				let Clip::Audio(clip) = self.clips.get_mut(&id).unwrap() else {
					panic!();
				};
				clip.fade_end.p = p;
			}
			NodeAction::ClipFadeEndToggleSymmetric(id) => {
				let Clip::Audio(clip) = self.clips.get_mut(&id).unwrap() else {
					panic!();
				};
				clip.fade_end.symmetric ^= true;
			}
			NodeAction::ClipStretchStartTo(id, pos) => {
				let Clip::Audio(clip) = self.clips.get_mut(&id).unwrap() else {
					panic!();
				};
				let fac = clip.position.stretch_start_to(pos, &state.transport);
				clip.fade_start.len /= fac;
				clip.fade_end.len /= fac;
				clip.stretch *= fac;
			}
			NodeAction::ClipStretchEndTo(id, pos) => {
				let Clip::Audio(clip) = self.clips.get_mut(&id).unwrap() else {
					panic!();
				};
				let fac = clip.position.stretch_end_to(pos, &state.transport);
				clip.fade_start.len /= fac;
				clip.fade_end.len /= fac;
				clip.stretch *= fac;
			}
			NodeAction::ClipReverse(id) => {
				let Clip::Audio(clip) = self.clips.get_mut(&id).unwrap() else {
					panic!();
				};
				clip.stretch *= -1.0;
				clip.position.reverse(
					state.samples[&clip.sample].len(&state.transport),
					clip.stretch.abs(),
				);
				(clip.fade_start, clip.fade_end) = (clip.fade_end, clip.fade_start);
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
		let polyphony = self.voice_alloc.current_polyphony();
		if polyphony != self.last_polyphony {
			self.last_polyphony = polyphony;
			updates.push(Update::Polyphony(self.id(), polyphony));
		}

		self.channel.collect_updates(updates);
	}

	pub fn clear_updates(&mut self) {
		self.channel.clear_updates();
	}

	pub fn restart_all_plugins(&mut self) {
		self.channel.restart_all_plugins();
	}
}
