use crate::{Event, NodeAction, Update, daw_ctx::State};
use audio_graph::{NodeId, NodeImpl};
use bitflags::bitflags;
use clap_host::AudioProcessor;
use generic_daw_utils::ShiftMoveExt as _;
use std::f32::consts::{FRAC_PI_4, SQRT_2};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PanMode {
	Balance(f32),
	Stereo(f32, f32),
}

impl PanMode {
	pub fn pan(self, audio: &mut [f32], volume: f32, invert: bool) {
		fn split(pan: f32, fac: f32) -> (f32, f32) {
			let angle = (pan + 1.0) * FRAC_PI_4;
			let (sin, cos) = angle.sin_cos();
			(cos * fac, sin * fac)
		}

		let (audio, rest) = audio.as_chunks_mut();
		debug_assert!(rest.is_empty());

		match self {
			Self::Balance(pan) => {
				let (mut l, mut r) = split(pan, volume * SQRT_2);
				if invert {
					(l, r) = (-l, -r);
				}
				for [ls, rs] in audio {
					*ls *= l;
					*rs *= r;
				}
			}
			Self::Stereo(l, r) => {
				let (mut ll, mut lr) = split(l, volume);
				let (mut rl, mut rr) = split(r, volume);
				if invert {
					(ll, lr, rl, rr) = (-ll, -lr, -rl, -rr);
				}
				for [ls, rs] in audio {
					let ols = *ls;
					*ls = ls.mul_add(ll, *rs * rl);
					*rs = rs.mul_add(rr, ols * lr);
				}
			}
		}
	}
}

#[derive(Debug)]
struct Plugin {
	processor: AudioProcessor<Event>,
	mix: f32,
	enabled: bool,
}

impl Plugin {
	pub fn new(processor: AudioProcessor<Event>) -> Self {
		Self {
			processor,
			mix: 1.0,
			enabled: true,
		}
	}
}

bitflags! {
	#[derive(Clone, Copy, Debug)]
	pub struct Flags: u8 {
		const ENABLED = 1 << 0;
		const BYPASSED = 1 << 1;
		const POLARITY_INVERTED = 1 << 2;
	}
}

impl Flags {
	fn processing(self) -> bool {
		self.contains(Self::ENABLED) && !self.contains(Self::BYPASSED)
	}
}

#[derive(Debug)]
pub struct Channel {
	id: NodeId,
	plugins: Vec<Plugin>,
	volume: f32,
	pan: PanMode,
	flags: Flags,
}

impl NodeImpl for Channel {
	type Event = Event;
	type State = State;

	fn process(&mut self, state: &Self::State, audio: &mut [f32], events: &mut Vec<Self::Event>) {
		for plugin in &mut self.plugins {
			if self.flags.processing() && plugin.enabled {
				plugin.processor.process(audio, events, plugin.mix);
			} else {
				plugin.processor.flush(events);
			}

			let mut events = events
				.extract_if(.., |event| matches!(event, Event::ParamValue { .. }))
				.map(|event| {
					let Event::ParamValue { param_id, .. } = event else {
						unreachable!()
					};

					Update::Param(plugin.processor.id(), param_id)
				})
				.peekable();

			if events.peek().is_some() {
				state.updates.lock().unwrap().extend(events);
			}
		}

		if !self.flags.contains(Flags::ENABLED) {
			audio.fill(0.0);
			events.clear();
			return;
		}

		self.pan.pan(
			audio,
			self.volume,
			self.flags.contains(Flags::POLARITY_INVERTED),
		);

		let peaks = max_peaks(audio);
		if peaks.iter().any(|&peak| peak >= f32::EPSILON) {
			state
				.updates
				.lock()
				.unwrap()
				.push(Update::Peak(self.id, peaks));
		}
	}

	fn id(&self) -> NodeId {
		self.id
	}

	fn delay(&self) -> usize {
		self.plugins
			.iter()
			.filter(|entry| self.flags.processing() && entry.enabled)
			.map(|entry| entry.processor.delay())
			.sum()
	}

	fn expensive(&self) -> bool {
		self.flags.processing() && self.plugins.iter().any(|plugin| plugin.enabled)
	}
}

impl Channel {
	pub fn apply(&mut self, action: NodeAction) {
		match action {
			NodeAction::ChannelToggleEnabled => self.flags.toggle(Flags::ENABLED),
			NodeAction::ChannelToggleBypassed => self.flags.toggle(Flags::BYPASSED),
			NodeAction::ChannelTogglePolarity => self.flags.toggle(Flags::POLARITY_INVERTED),
			NodeAction::ChannelVolumeChanged(volume) => self.volume = volume,
			NodeAction::ChannelPanChanged(pan) => self.pan = pan,
			NodeAction::PluginLoad(processor) => self.plugins.push(Plugin::new(*processor)),
			NodeAction::PluginRemove(index) => _ = self.plugins.remove(index),
			NodeAction::PluginMoveTo(from, to) => self.plugins.shift_move(from, to),
			NodeAction::PluginToggleEnabled(index) => self.plugins[index].enabled ^= true,
			NodeAction::PluginMixChanged(index, mix) => self.plugins[index].mix = mix,
			_ => panic!(),
		}
	}

	pub fn reset(&mut self) {
		for plugin in &mut self.plugins {
			plugin.processor.reset();
		}
	}
}

impl Default for Channel {
	fn default() -> Self {
		Self {
			plugins: Vec::new(),
			id: NodeId::unique(),
			volume: 1.0,
			pan: PanMode::Balance(0.0),
			flags: Flags::ENABLED,
		}
	}
}

fn max_peaks(audio: &[f32]) -> [f32; 2] {
	fn max_peaks<const N: usize>(mut old: [f32; N], new: [f32; N]) -> [f32; N] {
		for (old, new) in old.iter_mut().zip(new) {
			if new > *old {
				*old = new;
			}
		}
		old
	}

	let (chunks_16, rest) = audio.as_chunks::<16>();
	let (chunks_2, rest) = rest.as_chunks::<2>();
	debug_assert!(rest.is_empty());

	chunks_16
		.iter()
		.map(|chunk| chunk.map(f32::abs))
		.reduce(max_peaks)
		.into_iter()
		.flat_map(|chunk| <[[_; 2]; 8]>::try_from(chunk.as_chunks().0).unwrap())
		.chain(chunks_2.iter().map(|chunk| chunk.map(f32::abs)))
		.reduce(max_peaks)
		.unwrap_or([0.0; _])
}
