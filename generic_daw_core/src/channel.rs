use crate::{
	AutomationLane, Event, NodeAction, NodeId, NodeImpl, Update, clap_host::AudioProcessor,
	daw_ctx::State,
};
use std::f32::consts::{FRAC_PI_4, SQRT_2};
use utils::{ShiftMoveExt as _, unique_id};

unique_id!(plugin);

pub use plugin::Id as PluginId;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PanMode {
	Balance(f32),
	Stereo(f32, f32),
}

impl PanMode {
	pub fn pan(self, audio: &mut [f32], volume: f32) {
		fn split(pan: f32, fac: f32) -> (f32, f32) {
			let angle = (pan + 1.0) * FRAC_PI_4;
			let (sin, cos) = angle.sin_cos();
			(cos * fac, sin * fac)
		}

		let audio = audio.as_chunks_mut().0;

		match self {
			Self::Balance(pan) => {
				let (l, r) = split(pan, volume * SQRT_2);
				for [ls, rs] in audio {
					*ls *= l;
					*rs *= r;
				}
			}
			Self::Stereo(l, r) => {
				let (ll, lr) = split(l, volume);
				let (rl, rr) = split(r, volume);
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
	id: PluginId,
	processor: AudioProcessor<Event>,
	lanes: Vec<AutomationLane>,
	mix: f32,
	enabled: bool,
}

impl Plugin {
	pub fn new(id: PluginId, processor: AudioProcessor<Event>) -> Self {
		Self {
			id,
			processor,
			lanes: Vec::new(),
			mix: 1.0,
			enabled: true,
		}
	}
}

#[derive(Debug)]
pub struct Channel {
	id: NodeId,
	plugins: Vec<Plugin>,
	volume: f32,
	pan: PanMode,
	enabled: bool,
	bypassed: bool,
	last_peaks: [f32; 2],
	updates: Vec<Update>,
}

impl NodeImpl for Channel {
	type Event = Event;
	type State = State;

	fn process(&mut self, state: &Self::State, audio: &mut [f32], events: &mut Vec<Self::Event>) {
		let processing = self.processing();

		for plugin in &mut self.plugins {
			for lane in &mut plugin.lanes {
				lane.process(state, events);
			}

			if processing && plugin.enabled {
				plugin.processor.process(audio, events, plugin.mix);
			} else {
				debug_assert!(
					events
						.iter()
						.all(|event| matches!(event, Event::ParamValue { .. }))
				);

				plugin.processor.flush(events);
			}

			for event in events.drain(..) {
				if let Event::ParamValue { param_id, .. } = event {
					self.updates.push(Update::Param(plugin.id, param_id));
				}
			}
		}

		let peaks = if self.enabled {
			self.pan.pan(audio, self.volume);

			max_peaks(audio).map(|x| if x < f32::EPSILON { 0.0 } else { x })
		} else {
			audio.fill(0.0);
			events.clear();

			[0.0, 0.0]
		};

		if peaks != self.last_peaks {
			self.last_peaks = peaks;
			self.updates.push(Update::Peak(self.id, peaks));
		}
	}

	fn id(&self) -> NodeId {
		self.id
	}

	fn delay(&self) -> usize {
		self.plugins
			.iter()
			.filter(|plugin| self.processing() && plugin.enabled)
			.map(|plugin| plugin.processor.delay())
			.sum()
	}

	fn expensive(&self) -> bool {
		self.plugins
			.iter()
			.any(|plugin| self.processing() && plugin.enabled)
	}
}

impl Channel {
	pub fn apply(&mut self, action: NodeAction) {
		match action {
			NodeAction::ChannelToggleEnabled => self.enabled ^= true,
			NodeAction::ChannelToggleBypassed => self.bypassed ^= true,
			NodeAction::ChannelVolumeChanged(volume) => self.volume = volume,
			NodeAction::ChannelPanChanged(pan) => self.pan = pan,
			NodeAction::PluginLoad(id, processor) => self.plugins.push(Plugin::new(id, *processor)),
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

	pub fn collect_updates(&mut self, updates: &mut Vec<Update>) {
		updates.append(&mut self.updates);
	}

	#[must_use]
	pub fn enabled(&self) -> bool {
		self.enabled
	}

	fn processing(&self) -> bool {
		self.enabled && !self.bypassed
	}
}

impl Default for Channel {
	fn default() -> Self {
		Self {
			plugins: Vec::new(),
			id: NodeId::unique(),
			volume: 1.0,
			pan: PanMode::Balance(0.0),
			enabled: true,
			bypassed: false,
			last_peaks: [0.0; 2],
			updates: Vec::new(),
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
