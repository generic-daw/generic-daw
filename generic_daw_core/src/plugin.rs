use crate::{
	Event, Node, PushSlot, Update,
	audio_thread::{Inject, State},
};
use audio_graph::Injector;
use clap_host::{AudioThread, ClapId, RenderMode, events::EventFlags};
use std::collections::HashMap;
use utils::unique_id;

unique_id!(plugin);

pub use plugin::Id as PluginId;

#[derive(Debug)]
pub enum AudioThreadMessage {
	Restart(AudioProcessor),
	Deactivate(AudioProcessor),
	Destroy(AudioProcessor),
}

#[derive(Debug)]
pub struct AudioProcessor {
	clap: AudioThread,
	updates: HashMap<ClapId, f32>,
}

impl AudioProcessor {
	#[must_use]
	pub fn new(clap: AudioThread) -> Self {
		Self {
			updates: HashMap::with_capacity(clap.param_count()),
			clap,
		}
	}

	#[must_use]
	pub fn into_clap(self) -> AudioThread {
		self.clap
	}
}

#[derive(Debug)]
pub struct Plugin {
	pub processor: AudioProcessor,
	pub sender: oneshot::Sender<AudioThreadMessage>,
}

#[derive(Debug)]
pub struct PluginSlot {
	id: PluginId,
	plugin: PushSlot<PushSlot<Plugin>>,
	mix: f32,
}

impl Drop for PluginSlot {
	fn drop(&mut self) {
		if let Some(plugin) = self.plugin.as_mut().and_then(PushSlot::take) {
			plugin
				.sender
				.send(AudioThreadMessage::Destroy(plugin.processor))
				.unwrap();
		}
	}
}

impl PluginSlot {
	pub fn new(id: PluginId, plugin: PushSlot<PushSlot<Plugin>>) -> Self {
		Self {
			id,
			plugin,
			mix: 1.0,
		}
	}

	pub fn process(
		&mut self,
		state: &State,
		audio: &mut [[f32; 2]],
		events: &mut Vec<Event>,
		injector: &Injector<Node>,
	) -> usize {
		let Some(plugin) = (if state.render_mode == RenderMode::Realtime {
			self.plugin.try_recv()
		} else {
			self.plugin.recv()
		}) else {
			audio.fill([0.0; 2]);
			events.clear();
			return 0;
		};

		let plugin = match if state.render_mode == RenderMode::Realtime {
			plugin.try_recv()
		} else {
			plugin.as_mut()
		} {
			Some(plugin) if state.render_mode == RenderMode::Realtime => plugin,
			Some(plugin) if plugin.processor.clap.needs_restart() => {
				self.restart();

				let Some(plugin) = self.plugin.recv() else {
					audio.fill([0.0; 2]);
					events.clear();
					return 0;
				};

				match plugin.as_mut() {
					Some(plugin) => plugin,
					None => return 0,
				}
			}
			Some(plugin) => plugin,
			None => return 0,
		};

		plugin.processor.clap.push_all(events.drain(..));

		plugin.processor.clap.process::<Event>(
			audio,
			|event| {
				if let Event::ParamValue {
					param_id, value, ..
				} = event
				{
					plugin.processor.updates.insert(param_id, value);
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
			self.mix,
		);

		plugin.processor.clap.latency()
	}

	pub fn reset(&mut self) {
		if let Some(plugin) = self.plugin.as_mut().and_then(|slot| slot.as_mut()) {
			plugin.processor.clap.reset();
		}
	}

	pub fn collect_updates(&mut self, updates: &mut Vec<Update>) {
		if let Some(plugin) = self.plugin.as_mut().and_then(|slot| slot.as_mut()) {
			updates.extend(
				plugin
					.processor
					.updates
					.drain()
					.map(|(param_id, value)| Update::Param(self.id, param_id, value)),
			);

			if plugin.processor.clap.needs_restart() {
				self.restart();
			}
		}
	}

	pub fn mix_changed(&mut self, mix: f32) {
		self.mix = mix;
	}

	pub fn param_changed(&mut self, param_id: ClapId, value: f32) {
		if let Some(plugin) = self.plugin.as_mut().and_then(|slot| slot.as_mut()) {
			plugin.processor.clap.push(Event::ParamValue {
				time: 0,
				param_id,
				value,
				flags: EventFlags::IS_LIVE,
			});
		}
	}

	pub fn deactivate(&mut self) {
		if let Some(plugin) = self.plugin.as_mut().and_then(PushSlot::take) {
			plugin
				.sender
				.send(AudioThreadMessage::Deactivate(plugin.processor))
				.unwrap();
		}
	}

	pub fn restart(&mut self) {
		if self.plugin.as_mut().and_then(PushSlot::as_mut).is_some() {
			let plugin = self.plugin.take().unwrap().take().unwrap();
			plugin
				.sender
				.send(AudioThreadMessage::Restart(plugin.processor))
				.unwrap();
		}
	}
}
