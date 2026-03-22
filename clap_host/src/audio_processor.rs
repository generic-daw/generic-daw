use crate::{
	EventImpl, MainThreadMessage, PluginDescriptor, audio_buffers::AudioBuffers,
	event_buffers::EventBuffers, host::Host, shared::CURRENT_THREAD_ID,
};
use clack_extensions::render::RenderMode;
use clack_host::prelude::*;
use log::{trace, warn};
use rtrb::Consumer;
use std::sync::atomic::Ordering::Relaxed;
use utils::{NoClone, NoDebug};

#[derive(Debug)]
pub enum AudioThreadMessage<Event: EventImpl> {
	Activated(NoDebug<PluginAudioProcessor<Host>>, Option<u32>),
	RenderMode(RenderMode),
	Event(Event),
}

#[derive(Debug)]
pub struct AudioProcessor<Event: EventImpl> {
	processor: Option<NoDebug<PluginAudioProcessor<Host>>>,
	descriptor: PluginDescriptor,
	audio_buffers: AudioBuffers,
	event_buffers: EventBuffers,
	consumer: Consumer<AudioThreadMessage<Event>>,
	needs_reset: bool,
	render_mode: RenderMode,
}

impl<Event: EventImpl> AudioProcessor<Event> {
	#[must_use]
	pub fn new(
		descriptor: PluginDescriptor,
		audio_buffers: AudioBuffers,
		event_buffers: EventBuffers,
		consumer: Consumer<AudioThreadMessage<Event>>,
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

	pub fn recv_events(&mut self, events: &mut Vec<Event>) {
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
					AudioThreadMessage::Event(event) => events.push(event),
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

	pub fn process(&mut self, audio: &mut [f32], events: &mut Vec<Event>, mix_level: f32) {
		let Some(processor) = &mut self.processor else {
			return;
		};

		if processor.access_shared_handler(|s| !s.needs_process.load(Relaxed))
			&& events.is_empty()
			&& audio.iter().all(|f| f.abs() < f32::EPSILON)
		{
			return self.flush(events);
		}

		match processor.ensure_processing_started() {
			Ok(started_processor) => {
				let (input_audio, mut output_audio, steady_time) =
					self.audio_buffers.read_in(audio);
				self.event_buffers.read_in(events);

				if match started_processor.process(
					&input_audio,
					&mut output_audio,
					&self.event_buffers.input_events.as_input(),
					&mut self.event_buffers.output_events.as_output(),
					Some(steady_time),
					None,
				) {
					Ok(ProcessStatus::Continue | ProcessStatus::Tail) => false,
					Ok(ProcessStatus::ContinueIfNotQuiet) => self.audio_buffers.are_outputs_quiet(),
					Ok(ProcessStatus::Sleep) => true,
					Err(err) => {
						warn!("{}: {err}", &self.descriptor);
						false
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
				self.flush(events);
			}
		}
	}

	pub fn flush(&mut self, events: &mut Vec<Event>) {
		let Some(processor) = &mut self.processor else {
			return;
		};

		if !processor.access_shared_handler(|s| s.needs_flush.swap(false, Relaxed))
			&& events.is_empty()
		{
			return;
		}

		if let Some(&params) = processor.access_shared_handler(|s| s.ext.params.get()) {
			self.event_buffers.read_in(events);

			params.flush_active(
				&mut processor.plugin_handle(),
				&self.event_buffers.input_events.as_input(),
				&mut self.event_buffers.output_events.as_output(),
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

impl<Event: EventImpl> Drop for AudioProcessor<Event> {
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
