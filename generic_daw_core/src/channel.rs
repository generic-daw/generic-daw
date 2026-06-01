use crate::{Event, Node, NodeAction, NodeId, Update, audio_thread::State};
use audio_graph::{
	Inject,
	thread_pool::{Injector, WorkList},
};
use clap_host::AudioThread;
use std::{
	convert::Infallible,
	f32::consts::{FRAC_PI_4, SQRT_2},
};
use utils::{ShiftMoveExt as _, unique_id};

unique_id!(plugin);

pub use plugin::Id as PluginId;

#[derive(Debug)]
pub struct ThreadPoolExecutor<'a>(clap_host::ThreadPoolExecutor<'a>);

impl WorkList for ThreadPoolExecutor<'_> {
	type Item = u32;
	type Scratch = ();
	type Inject = Infallible;

	fn next_item(&self) -> Option<Self::Item> {
		self.0.next_task()
	}

	fn do_work(
		&self,
		item: Self::Item,
		_scratch: &mut Self::Scratch,
		_injector: &Injector<Self::Inject>,
	) -> Option<Self::Item> {
		self.0.exec_task(item);
		None
	}
}

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
	volume: f32,
	pan: PanMode,
	enabled: bool,
	bypassed: bool,
	last_peaks: [f32; 2],
	updates: Vec<Update>,
}

impl Channel {
	pub fn process(
		&mut self,
		state: &State,
		audio: &mut [f32],
		events: &mut Vec<Event>,
		injector: &Injector<Inject<Node>>,
	) -> usize {
		let acc = self
			.updates
			.pop_if(|update| matches!(update, Update::Peaks(..)));

		let mut latency = 0;

		for plugin in &mut self.plugins {
			if let Some(processor) = &mut plugin.processor {
				if self.bypassed {
					processor.flush_active::<Event>(|event| {
						self.updates.extend(event.into_update(plugin.id));
					});
				} else {
					processor.process(
						audio,
						events,
						Some(&state.transport.as_clap()),
						Some(&mut |executor| {
							let task_count = executor.task_count() as usize;
							let executor = ThreadPoolExecutor(executor);
							injector.inject(&executor, task_count);
						}),
						plugin.mix,
					);

					latency += processor.latency();

					events.retain(|&event| {
						let update = event.into_update(plugin.id);
						self.updates.extend(update);
						update.is_none()
					});
				}

				if processor.needs_restart() {
					plugin.processor.take().unwrap().restart();
				}
			}
		}

		self.pan.pan(audio, self.volume);
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
			audio.fill(0.0);
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
			NodeAction::ChannelToggleEnabled => self.enabled ^= true,
			NodeAction::ChannelToggleBypassed => self.bypassed ^= true,
			NodeAction::ChannelVolumeChanged(volume) => self.volume = volume,
			NodeAction::ChannelPanChanged(pan) => self.pan = pan,
			NodeAction::PluginAdd(id) => self.plugins.push(Plugin::new(id)),
			NodeAction::PluginRemove(index) => _ = self.plugins.remove(index),
			NodeAction::PluginActivate(index, processor) => {
				debug_assert!(self.plugins[index].processor.is_none());
				self.plugins[index].processor = Some(*processor);
			}
			NodeAction::PluginDeactivate(index) => {
				self.plugins[index].processor.take().unwrap().deactivate();
			}
			NodeAction::PluginMoveTo(from, to) => self.plugins.shift_move(from, to),
			NodeAction::PluginMixChanged(index, mix) => self.plugins[index].mix = mix,
			NodeAction::PluginParamChanged(index, param_id, value, cookie) => {
				if let Some(processor) = &mut self.plugins[index].processor {
					processor.push(Event::ParamValue {
						time: 0,
						param_id,
						value,
						cookie,
					});
				}
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
