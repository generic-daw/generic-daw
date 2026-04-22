use crate::{
	EventImpl, MainThreadMessage, PluginDescriptor, audio_buffers::AudioBuffers,
	event_buffers::EventBuffers, events::TransportEvent, host::Host, shared::CURRENT_THREAD_ID,
};
use clack_extensions::render::RenderMode;
use clack_host::prelude::*;
use log::{trace, warn};
use rtrb::Consumer;
use std::sync::atomic::Ordering::Relaxed;
use utils::{NoClone, NoDebug};

#[derive(Debug)]
pub enum AudioThreadMessage {
	Activated(NoDebug<PluginAudioProcessor<Host>>, Option<u32>),
	RenderMode(RenderMode),
}

#[derive(Debug)]
pub struct AudioProcessor {
	processor: Option<NoDebug<PluginAudioProcessor<Host>>>,
	descriptor: PluginDescriptor,
	audio_buffers: AudioBuffers,
	event_buffers: EventBuffers,
	consumer: Consumer<AudioThreadMessage>,
	needs_reset: bool,
	render_mode: RenderMode,
}

impl AudioProcessor {
	#[must_use]
	pub fn new(
		descriptor: PluginDescriptor,
		audio_buffers: AudioBuffers,
		event_buffers: EventBuffers,
		consumer: Consumer<AudioThreadMessage>,
	) -> Self {
		Self {
			processor: None,
			descriptor,
			audio_buffers,
			event_buffers,
			consumer,
			needs_reset: false,
			render_mode: RenderMode::Realtime,
		}
	}

	#[must_use]
	pub fn descriptor(&self) -> &PluginDescriptor {
		&self.descriptor
	}

	pub fn push_event(&mut self, event: impl EventImpl) {
		self.event_buffers.push(event);
	}

	pub fn recv_events(&mut self) {
		loop {
			while let Ok(msg) = self.consumer.pop() {
				trace!("{}: {msg:?}", self.descriptor);

				match msg {
					AudioThreadMessage::Activated(processor, latency) => {
						self.processor = Some(processor);
						if let Some(latency) = latency {
							self.audio_buffers.latency_changed(latency);
						}
					}
					AudioThreadMessage::RenderMode(render_mode) => self.render_mode = render_mode,
				}
			}

			if let Some(processor) = &mut self.processor {
				processor.access_shared_handler(|s| {
					CURRENT_THREAD_ID.with(|&id| s.audio_thread.store(id, Relaxed));
				});

				if std::mem::take(&mut self.needs_reset) {
					processor.reset();
					self.audio_buffers.reset();
				}
			}

			if self.render_mode == RenderMode::Realtime || self.processor.is_some() {
				break;
			}

			std::thread::yield_now();
		}
	}

	pub fn maybe_restart(&mut self) {
		if let Some(NoDebug(processor)) = self.processor.take() {
			if let Some(sender) = processor
				.access_shared_handler(|s| s.needs_restart.load(Relaxed).then(|| s.sender.clone()))
			{
				sender
					.send(MainThreadMessage::Restart(NoClone(NoDebug(
						processor.into_stopped(),
					))))
					.unwrap();
			} else {
				self.processor = Some(processor.into());
			}
		}
	}

	pub fn process(
		&mut self,
		audio: &mut [f32],
		events: &mut Vec<impl EventImpl>,
		transport: Option<&TransportEvent>,
		mix_level: f32,
	) {
		let Some(processor) = &mut self.processor else {
			self.audio_buffers.flush(audio, mix_level);
			return;
		};

		if processor.access_shared_handler(|s| !s.needs_process.load(Relaxed))
			&& events.is_empty()
			&& !audio.iter().any(|f| f.abs() >= f32::EPSILON)
		{
			self.audio_buffers.flush(audio, mix_level);
			self.flush(events);
			return;
		}

		match processor.ensure_processing_started() {
			Ok(started_processor) => {
				let (input_audio, mut output_audio, steady_time) =
					self.audio_buffers.read_in(audio);
				let (input_events, mut output_events) = self.event_buffers.read_in(events);

				if match started_processor.process(
					&input_audio,
					&mut output_audio,
					&input_events,
					&mut output_events,
					Some(steady_time),
					transport,
				) {
					Ok(ProcessStatus::Continue | ProcessStatus::Tail) => false,
					Ok(ProcessStatus::ContinueIfNotQuiet) => self.audio_buffers.are_outputs_quiet(),
					Ok(ProcessStatus::Sleep) => true,
					Err(err) => {
						warn!("{}: {err}", &self.descriptor);
						self.audio_buffers.flush(audio, mix_level);
						self.flush(events);
						return;
					}
				} {
					processor.ensure_processing_stopped();
					processor.access_shared_handler(|s| s.needs_process.store(false, Relaxed));
				}

				self.audio_buffers.write_out(audio, mix_level);
				self.event_buffers.write_out(events);

				processor.access_shared_handler(|s| s.needs_flush.store(false, Relaxed));
			}
			Err(err) => {
				warn!("{}: {err}", self.descriptor);
				self.audio_buffers.flush(audio, mix_level);
				self.flush(events);
			}
		}
	}

	pub fn flush(&mut self, events: &mut Vec<impl EventImpl>) {
		let Some(processor) = &mut self.processor else {
			return;
		};

		if !processor.access_shared_handler(|s| s.needs_flush.swap(false, Relaxed))
			&& events.is_empty()
		{
			return;
		}

		if let Some(&params) = processor.access_shared_handler(|s| s.ext.params.get()) {
			let (input_events, mut output_events) = self.event_buffers.read_in(events);

			params.flush_active(
				&mut processor.plugin_handle(),
				&input_events,
				&mut output_events,
			);

			self.event_buffers.write_out(events);
		}
	}

	pub fn reset(&mut self) {
		self.needs_reset = true;
	}

	#[must_use]
	pub fn delay(&self) -> usize {
		self.audio_buffers.delay()
	}
}

impl Drop for AudioProcessor {
	fn drop(&mut self) {
		if let Some(NoDebug(processor)) = self.processor.take() {
			processor
				.access_shared_handler(|s| {
					CURRENT_THREAD_ID.with(|&id| s.audio_thread.store(id, Relaxed));
					s.sender.clone()
				})
				.send(MainThreadMessage::Destroy(NoClone(NoDebug(
					processor.into_stopped(),
				))))
				.unwrap();
		}
	}
}
