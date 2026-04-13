use crate::{
	EventImpl, MainThreadMessage, PluginDescriptor, events::TransportEvent, host::Host,
	shared::CURRENT_THREAD_ID,
};
use clack_extensions::render::RenderMode;
use clack_host::prelude::*;
use log::{trace, warn};
use rtrb::Consumer;
use std::sync::atomic::Ordering::Relaxed;
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
	needs_reset: bool,
	render_mode: RenderMode,
}

impl AudioThread {
	#[must_use]
	pub fn new(descriptor: PluginDescriptor, consumer: Consumer<AudioThreadMessage>) -> Self {
		Self {
			processor: None,
			descriptor,
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
		if let Some(processor) = &mut self.processor {
			processor.access_handler_mut(|at| at.event_buffers.as_mut().unwrap().push(event));
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
		loop {
			while let Ok(msg) = self.consumer.pop() {
				trace!("{}: {msg:?}", self.descriptor);

				match msg {
					AudioThreadMessage::Activated(processor) => self.processor = Some(processor),
					AudioThreadMessage::RenderMode(render_mode) => self.render_mode = render_mode,
				}
			}

			if let Some(processor) = &mut self.processor
				&& std::mem::take(&mut self.needs_reset)
			{
				processor.reset();
				processor.access_handler_mut(|at| at.audio_buffers.as_mut().unwrap().reset());
			}

			if self.render_mode == RenderMode::Realtime {
				break;
			}

			self.maybe_deactivate();

			if self.processor.is_some() {
				break;
			}

			std::thread::yield_now();
		}
	}

	pub fn maybe_deactivate(&mut self) {
		if let Some(NoDebug(processor)) = self.processor.take() {
			if processor.access_shared_handler(|s| s.needs_deactivate.load(Relaxed)) {
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

		if processor.access_shared_handler(|s| !s.needs_process.load(Relaxed))
			&& events.is_empty()
			&& !audio.iter().any(|f| f.abs() >= f32::EPSILON)
		{
			self.flush(audio, events, mix_level);
			return;
		}

		match processor.ensure_processing_started() {
			Ok(started_processor) => {
				let mut audio_buffers =
					started_processor.access_handler_mut(|at| at.audio_buffers.take().unwrap());
				let mut event_buffers =
					started_processor.access_handler_mut(|at| at.event_buffers.take().unwrap());

				let (input_audio, mut output_audio, steady_time) = audio_buffers.read_in(audio);
				let (input_events, mut output_events) = event_buffers.read_in(events);

				if match started_processor.process(
					&input_audio,
					&mut output_audio,
					&input_events,
					&mut output_events,
					Some(steady_time),
					transport,
				) {
					Ok(ProcessStatus::Continue | ProcessStatus::Tail) => false,
					Ok(ProcessStatus::ContinueIfNotQuiet) => audio_buffers.are_outputs_quiet(),
					Ok(ProcessStatus::Sleep) => true,
					Err(err) => {
						warn!("{}: {err}", &self.descriptor);
						self.flush(audio, events, mix_level);
						return;
					}
				} {
					processor.ensure_processing_stopped();
					processor.access_shared_handler(|s| s.needs_process.store(false, Relaxed));
				}

				audio_buffers.write_out(audio, mix_level);
				event_buffers.write_out(events);

				processor.access_shared_handler(|s| s.needs_flush.store(false, Relaxed));

				processor.access_handler_mut(|at| at.audio_buffers = Some(audio_buffers));
				processor.access_handler_mut(|at| at.event_buffers = Some(event_buffers));
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

		processor.access_handler_mut(|at| {
			at.audio_buffers.as_mut().unwrap().flush(audio, mix_level);
		});

		if !processor.access_shared_handler(|s| s.needs_flush.swap(false, Relaxed))
			&& events.is_empty()
		{
			return;
		}

		if let Some(&params) = processor.access_shared_handler(|s| s.ext.params.get()) {
			let mut event_buffers =
				processor.access_handler_mut(|at| at.event_buffers.take().unwrap());

			let (input_events, mut output_events) = event_buffers.read_in(events);

			params.flush_active(
				&mut processor.plugin_handle(),
				&input_events,
				&mut output_events,
			);

			event_buffers.write_out(events);

			processor.access_handler_mut(|at| at.event_buffers = Some(event_buffers));
		}
	}

	pub fn reset(&mut self) {
		self.needs_reset = true;
	}

	#[must_use]
	pub fn delay(&self) -> usize {
		self.processor.as_ref().map_or(0, |processor| {
			processor.access_handler(|at| at.audio_buffers.as_ref().unwrap().delay())
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
