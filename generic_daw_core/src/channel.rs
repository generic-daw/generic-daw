use crate::{
	Channels, Event, Node, NodeAction, NodeId, Update,
	audio_thread::{Inject, State},
	scratch::Scratch,
};
use audio_graph::Injector;
use clap_host::{AudioThread, events::EventFlags};
use dsp::Utility;
use utils::{ShiftMoveExt as _, unique_id};

unique_id!(plugin);

pub use plugin::Id as PluginId;

#[derive(Debug)]
struct Plugin {
	id: PluginId,
	processor: Option<AudioThread>,
	mix: f32,
}

impl Drop for Plugin {
	fn drop(&mut self) {
		if let Some(processor) = self.processor.take() {
			processor.destroy();
		}
	}
}

impl Plugin {
	pub fn new(id: PluginId) -> Self {
		Self {
			id,
			processor: None,
			mix: 1.0,
		}
	}
}

#[derive(Debug)]
pub struct Channel {
	id: NodeId,
	plugins: Vec<Plugin>,
	utility: Utility,
	enabled: bool,
	bypassed: bool,
	output: Channels,
	last_peaks: [f32; 2],
	updates: Vec<Update>,
}

impl Channel {
	#[must_use]
	pub fn new(output: Channels) -> Self {
		Self {
			plugins: Vec::new(),
			id: NodeId::unique(),
			utility: Utility::default(),
			enabled: true,
			bypassed: false,
			output,
			last_peaks: [0.0; 2],
			updates: Vec::new(),
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
		let acc = self
			.updates
			.pop_if(|update| matches!(update, Update::Peaks(..)));

		let mut latency = 0;

		if self.bypassed {
			scratch.audio[..audio.len()].copy_from_slice(audio);
			scratch.events.clear();
			scratch.events.extend_from_slice(events);
		}

		for plugin in &mut self.plugins {
			if let Some(processor) = &mut plugin.processor {
				processor.push_all(events.drain(..));

				processor.process::<Event>(
					audio,
					|event| {
						if let Some(update) = event.into_update(plugin.id) {
							self.updates.push(update);
						} else {
							events.push(event);
						}
					},
					Some(&state.transport.as_clap()),
					Some(&mut |executor| {
						let task_count = executor.task_count() as usize;
						let executor = Inject(executor);
						injector.inject(&executor, task_count);
					}),
					plugin.mix,
				);

				if !self.bypassed {
					latency += processor.latency();
				}

				if processor.needs_restart() {
					plugin.processor.take().unwrap().restart();
				}
			}
		}

		if self.bypassed {
			audio.copy_from_slice(&scratch.audio[..audio.len()]);
			events.clear();
			events.extend_from_slice(&scratch.events);
		}

		self.utility.process(audio);
		let mut peaks = max_peaks(audio).map(|x| if x >= f32::EPSILON { x } else { 0.0 });

		if let Some(Update::Peaks(_, p)) = acc {
			peaks = [peaks[0].max(p[0]), peaks[1].max(p[1])];
		}

		if peaks != self.last_peaks {
			self.updates.push(Update::Peaks(self.id, peaks));
		}

		if self.enabled {
			latency
		} else {
			audio.fill([0.0; 2]);
			events.clear();
			0
		}
	}

	#[must_use]
	pub fn id(&self) -> NodeId {
		self.id
	}

	pub fn reset(&mut self) {
		for plugin in &mut self.plugins {
			if let Some(processor) = &mut plugin.processor {
				processor.reset();
			}
		}
	}

	pub fn apply(&mut self, action: NodeAction) {
		match action {
			NodeAction::OutputSetChannels(output) => self.output = output,
			NodeAction::ChannelToggleEnabled => self.enabled ^= true,
			NodeAction::ChannelToggleBypassed => self.bypassed ^= true,
			NodeAction::ChannelVolumeChanged(volume) => self.utility.volume = volume,
			NodeAction::ChannelPanChanged(pan) => self.utility.pan = pan,
			NodeAction::PluginInsert(index, id) => self.plugins.insert(index, Plugin::new(id)),
			NodeAction::PluginRemove(index) => _ = self.plugins.remove(index),
			NodeAction::PluginActivate(index, processor) => {
				debug_assert!(self.plugins[index].processor.is_none());
				self.plugins[index].processor = Some(*processor);
			}
			NodeAction::PluginDeactivate(index) => {
				if let Some(processor) = self.plugins[index].processor.take() {
					processor.deactivate();
				}
			}
			NodeAction::PluginMoveTo(from, to) => self.plugins.shift_move(from, to),
			NodeAction::PluginMixChanged(index, mix) => self.plugins[index].mix = mix,
			NodeAction::PluginParamChanged(index, param_id, value) => {
				if let Some(processor) = &mut self.plugins[index].processor {
					processor.push(Event::ParamValue {
						time: 0,
						param_id,
						value,
						flags: EventFlags::IS_LIVE,
					});
				}
			}
			_ => panic!("{action:?}"),
		}
	}

	pub fn collect_updates(&mut self, updates: &mut Vec<Update>) {
		if let Some(&Update::Peaks(_, peaks)) = self.updates.last() {
			debug_assert_ne!(self.last_peaks, peaks);
			self.last_peaks = peaks;
		}

		updates.append(&mut self.updates);
	}

	pub fn clear_updates(&mut self) {
		self.updates.clear();
	}

	#[must_use]
	pub fn output(&self) -> Channels {
		self.output
	}

	pub fn restart_all_plugins(&mut self) {
		for plugin in &mut self.plugins {
			if let Some(processor) = plugin.processor.take() {
				processor.restart();
			}
		}
	}
}

fn max_peaks(audio: &[[f32; 2]]) -> [f32; 2] {
	fn max_peaks<const N: usize>(mut old: [f32; N], new: [f32; N]) -> [f32; N] {
		for (old, new) in old.iter_mut().zip(new) {
			if new > *old {
				*old = new;
			}
		}
		old
	}

	let (chunks_16, rest) = audio.as_flattened().as_chunks::<16>();
	let (chunks_2, rest) = rest.as_chunks::<2>();
	debug_assert!(rest.is_empty());

	chunks_16
		.iter()
		.map(|chunk| chunk.map(f32::abs))
		.reduce(max_peaks)
		.into_iter()
		.flat_map(|chunk| *chunk.as_chunks().0.as_array::<8>().unwrap())
		.chain(chunks_2.iter().map(|chunk| chunk.map(f32::abs)))
		.reduce(max_peaks)
		.unwrap_or([0.0; 2])
}
