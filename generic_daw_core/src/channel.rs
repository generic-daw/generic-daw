use crate::{
	Event, NodeAction, NodeId, NodeImpl, Update, audio_thread::State, clap_host::AudioThread,
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
					*ls = *ls * ll + *rs * rl;
					*rs = *rs * rr + ols * lr;
				}
			}
		}
	}

	#[must_use]
	pub const fn is_balance(self) -> bool {
		matches!(self, Self::Balance(..))
	}
}

#[derive(Debug)]
struct Plugin {
	id: PluginId,
	processor: AudioThread,
	mix: f32,
}

impl Plugin {
	pub fn new(id: PluginId, processor: AudioThread) -> Self {
		Self {
			id,
			processor,
			mix: 1.0,
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
		let acc = self
			.updates
			.pop_if(|update| matches!(update, Update::Peaks(..)));

		for plugin in &mut self.plugins {
			plugin.processor.maybe_activate();
			plugin.processor.set_audio_thread();
			plugin.processor.maybe_deactivate();

			if self.enabled && !self.bypassed {
				plugin.processor.process(
					audio,
					events,
					Some(&state.transport.as_clap()),
					plugin.mix,
				);
			} else {
				plugin.processor.flush(audio, events, plugin.mix);
			}

			plugin.processor.maybe_restart();

			events.retain(|&event| {
				if let Event::ParamValue {
					param_id, value, ..
				} = event
				{
					self.updates.push(Update::Param(plugin.id, param_id, value));
					false
				} else {
					true
				}
			});
		}

		let mut peaks = if self.enabled {
			self.pan.pan(audio, self.volume);

			max_peaks(audio).map(|x| if x >= f32::EPSILON { x } else { 0.0 })
		} else {
			audio.fill(0.0);
			events.clear();

			[0.0, 0.0]
		};

		if let Some(Update::Peaks(_, p)) = acc {
			peaks = [peaks[0].max(p[0]), peaks[1].max(p[1])];
		}

		if peaks != self.last_peaks {
			self.updates.push(Update::Peaks(self.id, peaks));
		}
	}

	fn id(&self) -> NodeId {
		self.id
	}

	fn latency(&self) -> usize {
		if self.enabled && !self.bypassed {
			self.plugins
				.iter()
				.map(|plugin| plugin.processor.latency())
				.sum()
		} else {
			0
		}
	}

	fn reset(&mut self) {
		for plugin in &mut self.plugins {
			plugin.processor.reset();
		}
	}
}

impl Channel {
	pub fn apply(&mut self, action: NodeAction) {
		match action {
			NodeAction::ChannelToggleEnabled => self.enabled ^= true,
			NodeAction::ChannelToggleBypassed => self.bypassed ^= true,
			NodeAction::ChannelVolumeChanged(volume) => self.volume = volume,
			NodeAction::ChannelPanChanged(pan) => self.pan = pan,
			NodeAction::PluginAdd(id, processor) => self.plugins.push(Plugin::new(id, *processor)),
			NodeAction::PluginRemove(index) => _ = self.plugins.remove(index),
			NodeAction::PluginMoveTo(from, to) => self.plugins.shift_move(from, to),
			NodeAction::PluginMixChanged(index, mix) => self.plugins[index].mix = mix,
			NodeAction::PluginParamChanged(index, param_id, value, cookie) => {
				self.plugins[index].processor.push_event(Event::ParamValue {
					time: 0,
					param_id,
					value,
					cookie,
				});
			}
			_ => panic!(),
		}
	}

	pub fn collect_updates(&mut self, updates: &mut Vec<Update>) {
		if let Some(&Update::Peaks(_, peaks)) = self.updates.last() {
			debug_assert_ne!(self.last_peaks, peaks);
			self.last_peaks = peaks;
		}

		updates.append(&mut self.updates);
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
			last_peaks: [0.0, 0.0],
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
		.flat_map(|chunk| *chunk.as_chunks().0.as_array::<8>().unwrap())
		.chain(chunks_2.iter().map(|chunk| chunk.map(f32::abs)))
		.reduce(max_peaks)
		.unwrap_or([0.0; _])
}
