use crate::{Action, daw_ctx::State, event::Event};
use audio_graph::{NodeId, NodeImpl};
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

#[derive(Debug)]
pub struct Mixer {
	id: NodeId,
	plugins: Vec<Plugin>,
	volume: f32,
	pan: f32,
	enabled: bool,
}

impl NodeImpl for Mixer {
	type Event = Event;
	type State = State;

	fn process(
		&mut self,
		state: &mut Self::State,
		audio: &mut [f32],
		events: &mut Vec<Self::Event>,
	) {
		if !self.enabled {
			for s in audio {
				*s = 0.0;
			}
			events.clear();

			return;
		}

		for entry in &mut self.plugins {
			if entry.enabled {
				entry.processor.process(audio, events, entry.mix);
			} else {
				entry.processor.flush(events);
			}
		}

		let [lpan, rpan] = pan(self.pan).map(|s| s * self.volume);

		let peaks = peaks(audio, lpan, rpan);
		if peaks != [0.0; 2] {
			state.update.peaks.push((self.id, peaks));
		}
	}

	fn id(&self) -> NodeId {
		self.id
	}

	fn reset(&mut self) {
		for plugin in &mut self.plugins {
			plugin.processor.reset();
		}
	}

	fn delay(&self) -> usize {
		self.plugins
			.iter()
			.filter(|entry| entry.enabled)
			.map(|entry| entry.processor.delay())
			.sum()
	}
}

impl Mixer {
	pub fn apply(&mut self, action: Action) {
		match action {
			Action::NodeToggleEnabled => self.enabled ^= true,
			Action::NodeVolumeChanged(volume) => self.volume = volume,
			Action::NodePanChanged(pan) => self.pan = pan,
			Action::PluginLoad(processor) => self.plugins.push(Plugin::new(*processor)),
			Action::PluginRemove(index) => _ = self.plugins.remove(index),
			Action::PluginMoved(from, to) => self.plugins.shift_move(from, to),
			Action::PluginToggleEnabled(index) => self.plugins[index].enabled ^= true,
			Action::PluginMixChanged(index, mix) => self.plugins[index].mix = mix,
			_ => panic!(),
		}
	}
}

impl Default for Mixer {
	fn default() -> Self {
		Self {
			plugins: Vec::new(),
			id: NodeId::unique(),
			volume: 1.0,
			pan: 0.0,
			enabled: true,
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

	let (chunks_8, rest) = audio.as_chunks_mut::<8>();
	let (chunks_4, rest) = rest.as_chunks_mut::<4>();
	let (chunks_2, rest) = rest.as_chunks_mut::<2>();
	debug_assert!(rest.is_empty());

	chunks_8
		.iter_mut()
		.map(|chunk| pan_abs(chunk, lpan, rpan))
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
