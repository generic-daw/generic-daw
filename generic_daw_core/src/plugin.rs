use crate::{
	Event, Node, PushSlot,
	audio_thread::{Inject, State},
};
use audio_graph::Injector;
use clap_host::{AudioThread, ClapId, RenderMode, events::EventFlags};
use log::warn;
use rtrb::{Consumer, Producer, RingBuffer};
use std::collections::HashMap;

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
	producer: Producer<(ClapId, f32)>,
}

impl AudioProcessor {
	#[must_use]
	pub fn create(clap: AudioThread) -> (Self, Consumer<(ClapId, f32)>) {
		let (producer, consumer) = RingBuffer::new(
			(clap.param_count() * clap.config().sample_rate as usize)
				.div_ceil(clap.config().max_frames_count as usize),
		);
		(
			Self {
				updates: HashMap::with_capacity(clap.param_count()),
				clap,
				producer,
			},
			consumer,
		)
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
	plugin: PushSlot<Option<Plugin>>,
	mix: f32,
}

impl Drop for PluginSlot {
	fn drop(&mut self) {
		self.destroy();
	}
}

impl PluginSlot {
	pub fn new(plugin: PushSlot<Option<Plugin>>) -> Self {
		Self { plugin, mix: 1.0 }
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

		let plugin = match plugin {
			Some(plugin) if state.render_mode == RenderMode::Realtime => plugin,
			Some(plugin) if plugin.processor.clap.needs_restart() => {
				self.restart();

				let Some(plugin) = self.plugin.recv() else {
					audio.fill([0.0; 2]);
					events.clear();
					return 0;
				};

				match plugin {
					Some(plugin) => plugin,
					None => return 0,
				}
			}
			Some(plugin) => plugin,
			None => return 0,
		};

		debug_assert_eq!(
			plugin.processor.clap.config().sample_rate as u32,
			state.transport.sample_rate.get()
		);

		debug_assert_eq!(
			plugin.processor.clap.config().max_frames_count,
			state.transport.frames.get()
		);

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

		if state.render_mode == RenderMode::Realtime {
			for (param_id, value) in plugin.processor.updates.extract_if(|_, _| true) {
				if plugin.processor.producer.push((param_id, value)).is_err() {
					warn!("full ring buffer");
					plugin.processor.updates.insert(param_id, value);
					break;
				}
			}
		}

		let latency = plugin.processor.clap.latency();

		if plugin.processor.clap.needs_restart() {
			self.restart();
		}

		latency
	}

	pub fn reset(&mut self) {
		if let Some(plugin) = self.plugin.as_mut().and_then(|slot| slot.as_mut()) {
			plugin.processor.clap.reset();
		}
	}

	pub fn mix_changed(&mut self, mix: f32) {
		self.mix = mix;
	}

	pub fn param_changed(&mut self, param_id: ClapId, value: f32) {
		if let Some(plugin) = self.plugin.as_mut().and_then(|slot| slot.as_mut()) {
			plugin.processor.updates.remove(&param_id);
			plugin.processor.clap.push(Event::ParamValue {
				time: 0,
				param_id,
				value,
				flags: EventFlags::IS_LIVE,
			});
		}
	}

	pub fn activate(&mut self, plugin: Plugin) {
		*self.plugin.as_mut().unwrap() = Some(plugin);
	}

	pub fn deactivate(&mut self) {
		if let Some(plugin) = self.plugin.as_mut().and_then(Option::take) {
			plugin
				.sender
				.send(AudioThreadMessage::Deactivate(plugin.processor))
				.unwrap();
		}
	}

	pub fn restart(&mut self) {
		if self.plugin.as_mut().is_some_and(|plugin| plugin.is_some()) {
			let plugin = self.plugin.take().flatten().unwrap();
			plugin
				.sender
				.send(AudioThreadMessage::Restart(plugin.processor))
				.unwrap();
		}
	}

	pub fn destroy(&mut self) {
		if let Some(plugin) = self.plugin.as_mut().and_then(Option::take) {
			plugin
				.sender
				.send(AudioThreadMessage::Destroy(plugin.processor))
				.unwrap();
		}
	}
}
