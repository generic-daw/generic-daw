use crate::{
	Channels, Event, Node, NodeAction, NodeId, Update,
	audio_thread::{Scratch, State},
	plugin::PluginSlot,
};
use audio_graph::Injector;
use dsp::Utility;
use utils::ShiftMoveExt as _;

#[derive(Debug)]
pub struct Channel {
	id: NodeId,
	plugins: Vec<PluginSlot>,
	utility: Utility,
	enabled: bool,
	bypassed: bool,
	output: Channels,
	peaks: [f32; 2],
	last_peaks: [f32; 2],
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
			peaks: [0.0; 2],
			last_peaks: [0.0; 2],
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
		let mut latency = 0;

		if self.bypassed {
			scratch.audio[..audio.len()].copy_from_slice(audio);
			scratch.events.clone_from(events);
		}

		for plugin in &mut self.plugins {
			latency += plugin.process(state, audio, events, injector);
		}

		if self.bypassed {
			latency = 0;
			audio.copy_from_slice(&scratch.audio[..audio.len()]);
			events.clone_from(&scratch.events);
		}

		self.utility.process(audio);
		let peaks = max_peaks(audio).map(|x| if x >= f32::EPSILON { x } else { 0.0 });
		self.peaks = [self.peaks[0].max(peaks[0]), self.peaks[1].max(peaks[1])];

		if !self.enabled {
			latency = 0;
			audio.fill([0.0; 2]);
			events.clear();
		}

		latency
	}

	#[must_use]
	pub fn id(&self) -> NodeId {
		self.id
	}

	pub fn reset(&mut self) {
		for plugin in &mut self.plugins {
			plugin.reset();
		}
	}

	pub fn apply(&mut self, action: NodeAction) {
		match action {
			NodeAction::OutputSetChannels(output) => self.output = output,
			NodeAction::ChannelToggleEnabled => self.enabled ^= true,
			NodeAction::ChannelToggleBypassed => self.bypassed ^= true,
			NodeAction::ChannelVolumeChanged(volume) => self.utility.volume = volume,
			NodeAction::ChannelPanChanged(pan) => self.utility.pan = pan,
			NodeAction::PluginInsert(index, id, plugin) => {
				self.plugins.insert(index, PluginSlot::new(id, *plugin));
			}
			NodeAction::PluginRemove(index) => _ = self.plugins.remove(index),
			NodeAction::PluginActivate(index, plugin) => self.plugins[index].activate(*plugin),
			NodeAction::PluginDeactivate(index) => self.plugins[index].deactivate(),
			NodeAction::PluginMoveTo(from, to) => self.plugins.shift_move(from, to),
			NodeAction::PluginMixChanged(index, mix) => self.plugins[index].mix_changed(mix),
			NodeAction::PluginParamChanged(index, param_id, value) => {
				self.plugins[index].param_changed(param_id, value);
			}
			_ => panic!("{action:?}"),
		}
	}

	pub fn collect_updates(&mut self, updates: &mut Vec<Update>) {
		let peaks = std::mem::take(&mut self.peaks);
		if peaks != self.last_peaks {
			self.last_peaks = peaks;
			updates.push(Update::Peaks(self.id(), peaks));
		}

		for plugin in &mut self.plugins {
			plugin.collect_updates(updates);
		}
	}

	#[must_use]
	pub fn output(&self) -> Channels {
		self.output
	}

	pub fn restart_all_plugins(&mut self) {
		for plugin in &mut self.plugins {
			plugin.restart();
		}
	}
}

fn max_peaks(audio: &[[f32; 2]]) -> [f32; 2] {
	fn max_peaks<const N: usize>(mut old: [f32; N], new: [f32; N]) -> [f32; N] {
		for (old, new) in old.iter_mut().zip(new) {
			*old = if new > *old { new } else { *old };
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
		.unwrap_or_default()
}
