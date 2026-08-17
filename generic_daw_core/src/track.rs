use crate::{
	Channel, Channels, Clip, ClipId, Event, MidiAction, MidiNote, Node, NodeAction, NodeId, Update,
	audio_thread::State, midi_clip::VoiceId, scratch::Scratch, voice_alloc::VoiceAlloc,
};
use audio_graph::Injector;
use clap_host::{RenderMode, events::Match};
use log::warn;
use rtrb::Producer;
use std::collections::HashMap;

#[derive(Debug)]
pub struct Track {
	clips: HashMap<ClipId, Clip>,
	voice_alloc: VoiceAlloc<VoiceId, MidiNote>,
	last_polyphony: usize,
	input: Channels,
	audio_producer: Option<Producer<[f32; 2]>>,
	midi_producer: Option<Producer<MidiAction>>,
	channel: Channel,
}

impl Track {
	#[must_use]
	pub fn new(input: Channels, output: Channels) -> Self {
		Self {
			clips: HashMap::new(),
			voice_alloc: VoiceAlloc::default(),
			last_polyphony: 0,
			input,
			audio_producer: None,
			midi_producer: None,
			channel: Channel::new(output),
		}
	}

	pub fn process(
		&mut self,
		state: &State,
		audio: &mut [[f32; 2]],
		events: &mut Vec<Event>,
		scratch: &mut Scratch,
		injector: &Injector<Node>,
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

		if state.render_mode == RenderMode::Realtime {
			let iter = state
				.midi_input
				.iter()
				.filter(|action| self.input.midi & (1 << action.channel().as_int()) != 0);

			if self.input.enable_midi {
				events.extend(iter.clone().map(|action| match action {
					MidiAction::NoteOn(channel, key, velocity) => Event::On {
						time: 0,
						key: key.as_int(),
						velocity: f32::from(velocity.as_int()) / 127.0,
						note_id: Match::Specific(
							i32::MAX.cast_unsigned() - 1 - u32::from(channel.as_int()),
						),
					},
					MidiAction::NoteOff(channel, key, velocity) => Event::Off {
						time: 0,
						key: key.as_int(),
						velocity: f32::from(velocity.as_int()) / 127.0,
						note_id: Match::Specific(
							i32::MAX.cast_unsigned() - 1 - u32::from(channel.as_int()),
						),
					},
				}));
			}

			if let Some(producer) = &mut self.midi_producer {
				for &event in iter {
					if producer.push(event).is_err() {
						warn!("full ring buffer");
						break;
					}
				}
			}

			if self.input.fits_in(state.transport.input_channels) {
				let audio = if self.input.enable_audio {
					&mut *audio
				} else if self.audio_producer.is_some() && state.transport.playing {
					&mut scratch.audio[..audio.len()]
				} else {
					&mut []
				};

				for ([l, r], frame) in audio.iter_mut().zip(
					state
						.audio_input
						.chunks_exact(state.transport.input_channels.into()),
				) {
					*l = frame[usize::from(self.input.left)];
					*r = frame[usize::from(self.input.right)];
				}

				if let Some(producer) = &mut self.audio_producer
					&& state.transport.playing
					&& let (_, rest) = producer.push_partial_slice(audio)
					&& !rest.is_empty()
				{
					warn!("full ring buffer");
				}
			}
		}

		if state.transport.playing {
			for clip in self.clips.values_mut() {
				clip.process(state, audio, events, &mut self.voice_alloc);
			}
		}

		self.channel
			.process(state, audio, events, scratch, injector)
	}

	#[must_use]
	pub fn id(&self) -> NodeId {
		self.channel.id()
	}

	pub fn reset(&mut self) {
		self.channel.reset();
	}

	pub fn apply(&mut self, action: NodeAction, state: &mut State) {
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
			NodeAction::InputSetChannels(input) => {
				if (self.input.left != input.left || self.input.right != input.right)
					&& self.audio_producer.is_some()
				{
					state
						.updates
						.push(Update::AudioInterrupted(self.channel.id()));
				}

				if self.input.midi != input.midi
					&& let Some(producer) = &mut self.midi_producer
				{
					state
						.updates
						.push(Update::MidiInterrupted(self.channel.id()));

					for (&(channel, key), &velocity) in &state.playing {
						if input.midi & (1 << channel.as_int()) != 0
							&& producer
								.push(MidiAction::NoteOn(channel, key, velocity))
								.is_err()
						{
							warn!("full ring buffer");
							break;
						}
					}
				}

				self.input = input;
			}
			NodeAction::InputSetAudioRecording(producer) => {
				self.audio_producer = producer;
				if self.audio_producer.is_none() {
					state
						.updates
						.push(Update::AudioInterrupted(self.channel.id()));
				}
			}
			NodeAction::InputSetMidiRecording(producer) => {
				self.midi_producer = producer;
				if let Some(producer) = &mut self.midi_producer {
					for (&(channel, key), &velocity) in &state.playing {
						if self.input.midi & (1 << channel.as_int()) != 0
							&& producer
								.push(MidiAction::NoteOn(channel, key, velocity))
								.is_err()
						{
							warn!("full ring buffer");
							break;
						}
					}
				} else {
					state
						.updates
						.push(Update::MidiInterrupted(self.channel.id()));
				}
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

	#[must_use]
	pub fn output(&self) -> Channels {
		self.channel.output()
	}

	pub fn restart_all_plugins(&mut self) {
		self.channel.restart_all_plugins();
	}

	#[must_use]
	pub fn from_channel(input: Channels, channel: Channel) -> Self {
		Self {
			clips: HashMap::new(),
			voice_alloc: VoiceAlloc::default(),
			last_polyphony: 0,
			input,
			audio_producer: None,
			midi_producer: None,
			channel,
		}
	}

	#[must_use]
	pub fn into_channel(track: Self) -> Channel {
		track.channel
	}
}
