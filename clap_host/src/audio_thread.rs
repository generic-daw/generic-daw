use crate::{
	EventImpl, MainThreadMessage, PluginDescriptor, events::TransportEvent, host::Host,
	shared::CURRENT_THREAD_ID,
};
use clack_extensions::{render::RenderMode, tail::TailLength};
use clack_host::prelude::*;
use log::{trace, warn};
use rtrb::Consumer;
use std::{cell::LazyCell, sync::atomic::Ordering::Relaxed};
use utils::{NoClone, NoDebug};

#[derive(Debug)]
pub enum AudioThreadMessage {
	Activated(NoDebug<PluginAudioProcessor<Host>>),
	RenderMode(RenderMode),
}

#[derive(Debug)]
pub struct AudioThread {
	processor: Option<NoDebug<PluginAudioProcessor<Host>>>,
	descriptor: PluginDescriptor,
	consumer: Consumer<AudioThreadMessage>,
	processing: bool,
	needs_reset: bool,
	render_mode: RenderMode,
	last_input: Option<u64>,
}

impl AudioThread {
	#[must_use]
	pub fn new(descriptor: PluginDescriptor, consumer: Consumer<AudioThreadMessage>) -> Self {
		Self {
			processor: None,
			descriptor,
			consumer,
			processing: false,
			needs_reset: false,
			render_mode: RenderMode::Realtime,
			last_input: None,
		}
	}

	#[must_use]
	pub fn descriptor(&self) -> &PluginDescriptor {
		&self.descriptor
	}

	pub fn push_event(&mut self, event: impl EventImpl) {
		if let Some(processor) = &mut self.processor {
			processor.access_handler_mut(|ap| ap.event_buffers.as_mut().unwrap().push(event));
		} else {
			warn!(
				"{}: received {:?} while deactivated",
				&self.descriptor, event
			);
		}
	}

	pub fn set_audio_thread(&self) {
		if let Some(processor) = &self.processor {
			processor.access_shared_handler(|s| {
				CURRENT_THREAD_ID.with(|&id| s.audio_thread.store(id, Relaxed));
			});
		}
	}

	pub fn maybe_activate(&mut self) {
		while let Ok(msg) = self.consumer.pop() {
			trace!("{}: {msg:?}", self.descriptor);

			match msg {
				AudioThreadMessage::Activated(processor) => self.processor = Some(processor),
				AudioThreadMessage::RenderMode(render_mode) => self.render_mode = render_mode,
			}
		}
	}

	pub fn maybe_deactivate(&mut self) {
		if let Some(NoDebug(processor)) = self.processor.take() {
			if processor.access_shared_handler(|s| s.request_deactivate.load(Relaxed)) {
				processor
					.access_shared_handler(|s| s.sender.clone())
					.send(MainThreadMessage::Deactivate(NoClone(NoDebug(
						processor.into_stopped(),
					))))
					.unwrap();
			} else {
				self.processor = Some(processor.into());
			}
		}
	}

	pub fn maybe_restart(&mut self) {
		if let Some(NoDebug(processor)) = self.processor.take() {
			if processor.access_shared_handler(|s| s.request_restart.load(Relaxed)) {
				processor
					.access_shared_handler(|s| s.sender.clone())
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
			return;
		};

		let audio_in = LazyCell::new(|| audio.iter().any(|f| f.abs() >= f32::EPSILON));
		let events_in = !events.is_empty();
		let request_process =
			processor.access_shared_handler(|s| s.request_process.swap(false, Relaxed));

		if !self.processing && !request_process && !events_in && !*audio_in {
			self.flush(audio, events, mix_level);
			return;
		}

		match processor.ensure_processing_started() {
			Ok(started_processor) => {
				let mut audio_buffers =
					started_processor.access_handler_mut(|ap| ap.audio_buffers.take().unwrap());
				let mut event_buffers =
					started_processor.access_handler_mut(|ap| ap.event_buffers.take().unwrap());

				if std::mem::take(&mut self.needs_reset) {
					started_processor.reset();
					audio_buffers.reset();
					self.last_input = None;
				}

				let (input_audio, mut output_audio, steady_time) = audio_buffers.read_in(audio);
				let (input_events, mut output_events) = event_buffers.read_in(events);

				if started_processor.access_shared_handler(|s| s.ext.tail.get().is_some())
					&& (request_process || events_in || *audio_in)
				{
					self.last_input = Some(steady_time);
				}

				let process_status = started_processor.process(
					&input_audio,
					&mut output_audio,
					&input_events,
					&mut output_events,
					Some(steady_time),
					transport,
				);

				self.processing = match process_status {
					Ok(ProcessStatus::Continue) => true,
					Ok(ProcessStatus::ContinueIfNotQuiet) => !audio_buffers.are_outputs_quiet(),
					Ok(ProcessStatus::Tail) => {
						match processor
							.access_shared_handler(|s| *s.ext.tail.get().unwrap())
							.get(&processor.plugin_handle())
						{
							TailLength::Infinite => true,
							TailLength::Finite(tail) => self.last_input.is_none_or(|last_input| {
								steady_time - last_input < u64::from(tail)
							}),
						}
					}
					Ok(ProcessStatus::Sleep) => false,
					Err(err) => {
						warn!("{}: {err}", &self.descriptor);
						self.flush(audio, events, mix_level);
						return;
					}
				};

				audio_buffers.write_out(audio, mix_level);
				event_buffers.write_out(events);

				processor.access_shared_handler(|s| s.request_flush.store(false, Relaxed));

				processor.access_handler_mut(|ap| ap.audio_buffers = Some(audio_buffers));
				processor.access_handler_mut(|ap| ap.event_buffers = Some(event_buffers));
			}
			Err(err) => {
				warn!("{}: {err}", self.descriptor);
				self.flush(audio, events, mix_level);
			}
		}
	}

	pub fn flush(&mut self, audio: &mut [f32], events: &mut Vec<impl EventImpl>, mix_level: f32) {
		let Some(processor) = &mut self.processor else {
			return;
		};

		processor.ensure_processing_stopped();

		processor
			.access_handler_mut(|ap| ap.audio_buffers.as_mut().unwrap().flush(audio, mix_level));

		let events_in = !events.is_empty();
		let request_flush =
			processor.access_shared_handler(|s| s.request_flush.swap(false, Relaxed));

		if !request_flush && !events_in {
			return;
		}

		if let Some(&params) = processor.access_shared_handler(|s| s.ext.params.get()) {
			let mut event_buffers =
				processor.access_handler_mut(|ap| ap.event_buffers.take().unwrap());

			let (input_events, mut output_events) = event_buffers.read_in(events);

			params.flush_active(
				&mut processor.plugin_handle(),
				&input_events,
				&mut output_events,
			);

			event_buffers.write_out(events);

			processor.access_handler_mut(|ap| ap.event_buffers = Some(event_buffers));
		}
	}

	pub fn reset(&mut self) {
		self.needs_reset = true;
	}

	#[must_use]
	pub fn latency(&self) -> usize {
		self.processor.as_ref().map_or(0, |processor| {
			processor.access_handler(|ap| ap.audio_buffers.as_ref().unwrap().latency())
		})
	}
}

impl Drop for AudioThread {
	fn drop(&mut self) {
		self.set_audio_thread();

		if let Some(NoDebug(processor)) = self.processor.take() {
			processor
				.access_shared_handler(|s| s.sender.clone())
				.send(MainThreadMessage::Destroy(NoClone(NoDebug(
					processor.into_stopped(),
				))))
				.unwrap();
		}
	}
}
