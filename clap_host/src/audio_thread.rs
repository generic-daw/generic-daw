use crate::{
	EventImpl, MainThreadMessage, ThreadPoolInjector, audio_buffers::AudioBuffers,
	event_buffers::EventBuffers, events::TransportEvent, host::Host, shared::CURRENT_THREAD_ID,
};
use clack_extensions::tail::TailLength;
use clack_host::prelude::*;
use log::warn;
use std::{cell::LazyCell, sync::atomic::Ordering::Relaxed};
use utils::{NoClone, NoDebug};

#[derive(Debug)]
pub struct AudioThread {
	pub(crate) processor: NoDebug<PluginAudioProcessor<Host>>,
	audio_buffers: AudioBuffers,
	event_buffers: EventBuffers,
	last_input: Option<u64>,
	processing: bool,
}

impl AudioThread {
	#[must_use]
	pub fn new(
		processor: StoppedPluginAudioProcessor<Host>,
		audio_buffers: AudioBuffers,
		event_buffers: EventBuffers,
	) -> Self {
		Self {
			processor: NoDebug(processor.into()),
			audio_buffers,
			event_buffers,
			last_input: None,
			processing: false,
		}
	}

	pub fn push_all(&mut self, events: impl IntoIterator<Item: EventImpl>) {
		self.event_buffers.push_all(events);
	}

	pub fn push(&mut self, event: impl EventImpl) {
		self.event_buffers.push(event);
	}

	#[must_use]
	pub fn needs_restart(&self) -> bool {
		self.processor
			.access_shared_handler(|s| s.request_restart.load(Relaxed))
	}

	pub fn process(
		&mut self,
		audio: &mut [[f32; 2]],
		events: &mut Vec<impl EventImpl>,
		transport: Option<&TransportEvent>,
		injector: Option<ThreadPoolInjector<'_>>,
		mix_level: f32,
	) {
		self.processor.access_shared_handler(|s| {
			CURRENT_THREAD_ID.with(|&id| s.audio_thread.store(id, Relaxed));
		});

		let steady_time = self.audio_buffers.read_in(audio);
		self.event_buffers.push_all(events.drain(..));

		let audio_in = LazyCell::new(|| !self.audio_buffers.are_inputs_quiet());
		let events_in = !self.event_buffers.are_inputs_empty();
		let request_process = self
			.processor
			.access_shared_handler(|s| s.request_process.swap(false, Relaxed));

		if !self.processing && !request_process && !events_in && !*audio_in {
			self.flush_process(audio, events, mix_level);
			return;
		}

		match self.processor.ensure_processing_started() {
			Ok(started_processor) => {
				if started_processor.access_shared_handler(|s| s.ext.tail.get().is_some())
					&& (request_process || events_in || *audio_in)
				{
					self.last_input = Some(steady_time);
				}

				let (input_audio, mut output_audio) = self.audio_buffers.prepare(audio.len());
				let (input_events, mut output_events) = self.event_buffers.prepare();

				started_processor.access_handler_mut(|ap| {
					ap.injector = injector
						.map(|injector|
							// SAFETY:
							// `injector` is a valid reference to a `ThreadPoolInjector` at least
							// until this function returns. `ap.injector` is overwritten after the
							// call to `process`, and `AudioProcessor` never creates any copies of
							// `ap.injector` that may outlive `process`. Any possible panics that
							// may happen during `process` are caught by clack.
							unsafe { std::mem::transmute(injector) })
						.map(NoDebug);
				});

				let process_status = started_processor.process(
					&input_audio,
					&mut output_audio,
					&input_events,
					&mut output_events,
					Some(steady_time),
					transport,
				);

				self.processor.access_handler_mut(|ap| ap.injector = None);

				self.processing = match process_status {
					Ok(ProcessStatus::Continue) => true,
					Ok(ProcessStatus::ContinueIfNotQuiet) => {
						!self.audio_buffers.are_outputs_quiet()
					}
					Ok(ProcessStatus::Tail) => {
						match self
							.processor
							.access_shared_handler(|s| *s.ext.tail.get().unwrap())
							.get(&self.processor.plugin_handle())
						{
							TailLength::Infinite => true,
							TailLength::Finite(tail) => self.last_input.is_none_or(|last_input| {
								steady_time - last_input < u64::from(tail)
							}),
						}
					}
					Ok(ProcessStatus::Sleep) => false,
					Err(err) => {
						warn!(
							"{}: {err}",
							self.processor.access_shared_handler(|s| &s.descriptor)
						);
						self.flush_process(audio, events, mix_level);
						return;
					}
				};

				self.audio_buffers.write_out(audio, mix_level);
				events.extend(self.event_buffers.output_events());
				self.event_buffers.reset();

				self.processor
					.access_shared_handler(|s| s.request_flush.store(false, Relaxed));
			}
			Err(err) => {
				warn!(
					"{}: {err}",
					self.processor.access_shared_handler(|s| &s.descriptor)
				);
				self.flush_process(audio, events, mix_level);
			}
		}
	}

	fn flush_process(
		&mut self,
		audio: &mut [[f32; 2]],
		events: &mut Vec<impl EventImpl>,
		mix_level: f32,
	) {
		debug_assert!(events.is_empty());

		self.processor.ensure_processing_stopped();

		self.audio_buffers.flush(audio, mix_level);

		let events_in = !events.is_empty();
		let request_flush = self
			.processor
			.access_shared_handler(|s| s.request_flush.swap(false, Relaxed));

		if !request_flush && !events_in {
			return;
		}

		if let Some(&params) = self.processor.access_shared_handler(|s| s.ext.params.get()) {
			let (input_events, mut output_events) = self.event_buffers.prepare();

			params.flush_active(
				&mut self.processor.plugin_handle(),
				&input_events,
				&mut output_events,
			);

			events.extend(self.event_buffers.output_events());
			self.event_buffers.reset();
		}
	}

	pub fn flush_active<Event: EventImpl>(&mut self, f: impl FnMut(Event)) {
		self.processor.access_shared_handler(|s| {
			CURRENT_THREAD_ID.with(|&id| s.audio_thread.store(id, Relaxed));
		});

		self.processor.ensure_processing_stopped();

		let events_in = !self.event_buffers.are_inputs_empty();
		let request_flush = self
			.processor
			.access_shared_handler(|s| s.request_flush.swap(false, Relaxed));

		if !request_flush && !events_in {
			return;
		}

		if let Some(&params) = self.processor.access_shared_handler(|s| s.ext.params.get()) {
			let (input_events, mut output_events) = self.event_buffers.prepare();

			params.flush_active(
				&mut self.processor.plugin_handle(),
				&input_events,
				&mut output_events,
			);

			self.event_buffers.output_events().for_each(f);
			self.event_buffers.reset();
		}
	}

	#[must_use]
	pub fn latency(&self) -> usize {
		self.audio_buffers.latency()
	}

	pub fn reset(&mut self) {
		self.processor.access_shared_handler(|s| {
			CURRENT_THREAD_ID.with(|&id| s.audio_thread.store(id, Relaxed));
		});
		self.processor.reset();
		self.audio_buffers.reset();
		self.event_buffers.reset();
		self.last_input = None;
		self.processing = false;
	}

	pub fn deactivate(self) {
		self.processor
			.access_shared_handler(|s| s.sender.clone())
			.send(MainThreadMessage::Deactivate(NoClone(self)))
			.unwrap();
	}

	pub fn restart(self) {
		self.processor
			.access_shared_handler(|s| s.sender.clone())
			.send(MainThreadMessage::Restart(NoClone(self)))
			.unwrap();
	}

	pub fn destroy(self) {
		self.processor
			.access_shared_handler(|s| s.sender.clone())
			.send(MainThreadMessage::Destroy(NoClone(self)))
			.unwrap();
	}
}
