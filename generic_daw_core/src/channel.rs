use crate::{
	Action,
	daw_ctx::{State, Update},
	event::Event,
};
use audio_graph::{NodeId, NodeImpl};
use bitflags::bitflags;
use clap_host::AudioProcessor;
use generic_daw_utils::ShiftMoveExt as _;
use std::f32::consts::{FRAC_PI_4, SQRT_2};

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
		const CHANNELS_SWAPPED = 1 << 3;
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
	pan: f32,
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

			events
				.extract_if(.., |event| matches!(event, Event::ParamValue { .. }))
				.map(|event| {
					let Event::ParamValue { param_id, .. } = event else {
						unreachable!()
					};

					Update::Param(plugin.processor.id(), param_id)
				})
				.for_each(|update| state.updates.push(update));
		}

		if !self.flags.contains(Flags::ENABLED) {
			audio.fill(0.0);
			events.clear();
			return;
		}

		if self.flags.contains(Flags::CHANNELS_SWAPPED) {
			for [l, r] in audio.as_chunks_mut().0 {
				(*l, *r) = (*r, *l);
			}
		}

		let [mut lpan, mut rpan] = pan(self.pan).map(|s| s * self.volume);

		if self.flags.contains(Flags::POLARITY_INVERTED) {
			lpan = -lpan;
			rpan = -rpan;
		}

		let peaks = peaks(audio, lpan, rpan);
		if peaks.iter().all(|&peak| peak >= f32::EPSILON) {
			state.updates.push(Update::Peak(self.id, peaks));
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
	pub fn apply(&mut self, action: Action) {
		match action {
			Action::ChannelToggleEnabled => self.flags.toggle(Flags::ENABLED),
			Action::ChannelToggleBypassed => self.flags.toggle(Flags::BYPASSED),
			Action::ChannelTogglePolarity => self.flags.toggle(Flags::POLARITY_INVERTED),
			Action::ChannelSwapChannels => self.flags.toggle(Flags::CHANNELS_SWAPPED),
			Action::ChannelVolumeChanged(volume) => self.volume = volume,
			Action::ChannelPanChanged(pan) => self.pan = pan,
			Action::PluginLoad(processor) => self.plugins.push(Plugin::new(*processor)),
			Action::PluginRemove(index) => _ = self.plugins.remove(index),
			Action::PluginMoved(from, to) => self.plugins.shift_move(from, to),
			Action::PluginToggleEnabled(index) => self.plugins[index].enabled ^= true,
			Action::PluginMixChanged(index, mix) => self.plugins[index].mix = mix,
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
			pan: 0.0,
			flags: Flags::ENABLED,
		}
	}
}

fn pan(pan: f32) -> [f32; 2] {
	let angle = (pan + 1.0) * FRAC_PI_4;
	let (sin, cos) = angle.sin_cos();
	[cos * SQRT_2, sin * SQRT_2]
}

fn peaks(audio: &mut [f32], lpan: f32, rpan: f32) -> [f32; 2] {
	fn pan_abs<const N: usize>(chunk: &mut [f32; N], lpan: f32, rpan: f32) -> [f32; N] {
		for (i, s) in chunk.iter_mut().enumerate() {
			*s *= if i % 2 == 0 { lpan } else { rpan }
		}
		chunk.map(f32::abs)
	}

	fn array_max<const N: usize>(mut old: [f32; N], new: [f32; N]) -> [f32; N] {
		for (old, new) in old.iter_mut().zip(new) {
			if new > *old {
				*old = new;
			}
		}
		old
	}

	let (chunks_16, rest) = audio.as_chunks_mut::<16>();
	let (chunks_8, rest) = rest.as_chunks_mut::<8>();
	let (chunks_4, rest) = rest.as_chunks_mut::<4>();
	let (chunks_2, rest) = rest.as_chunks_mut::<2>();
	debug_assert!(rest.is_empty());

	chunks_16
		.iter_mut()
		.map(|chunk| pan_abs(chunk, lpan, rpan))
		.reduce(array_max)
		.into_iter()
		.flat_map(|chunk| <[[_; 8]; 2]>::try_from(chunk.as_chunks().0).unwrap())
		.chain(chunks_8.iter_mut().map(|chunk| pan_abs(chunk, lpan, rpan)))
		.reduce(array_max)
		.into_iter()
		.flat_map(|chunk| <[[_; 4]; 2]>::try_from(chunk.as_chunks().0).unwrap())
		.chain(chunks_4.iter_mut().map(|chunk| pan_abs(chunk, lpan, rpan)))
		.reduce(array_max)
		.into_iter()
		.flat_map(|chunk| <[[_; 2]; 2]>::try_from(chunk.as_chunks().0).unwrap())
		.chain(chunks_2.iter_mut().map(|chunk| pan_abs(chunk, lpan, rpan)))
		.reduce(array_max)
		.unwrap_or([0.0; _])
}
