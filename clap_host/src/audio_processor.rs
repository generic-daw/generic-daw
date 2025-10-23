use crate::{
	EventImpl, MainThreadMessage, PluginDescriptor, PluginId, audio_buffers::AudioBuffers,
	event_buffers::EventBuffers, host::Host, shared::CURRENT_THREAD_ID,
};
use clack_host::process::{PluginAudioProcessor, ProcessStatus};
use generic_daw_utils::{NoClone, NoDebug};
use log::{trace, warn};
use rtrb::Consumer;
use std::{hint::spin_loop, sync::atomic::Ordering::Relaxed};

#[derive(Debug)]
pub enum AudioThreadMessage<Event: EventImpl> {
	Activated(NoDebug<PluginAudioProcessor<Host>>, u32),
	SetRealtime(bool),
	Event(Event),
}

#[derive(Debug)]
pub struct AudioProcessor<Event: EventImpl> {
	processor: Option<NoDebug<PluginAudioProcessor<Host>>>,
	descriptor: PluginDescriptor,
	id: PluginId,
	steady_time: u64,
	audio_buffers: AudioBuffers,
	event_buffers: EventBuffers,
	consumer: Consumer<AudioThreadMessage<Event>>,
	needs_reset: bool,
	realtime: bool,
}

impl<Event: EventImpl> AudioProcessor<Event> {
	#[must_use]
	pub fn new(
		descriptor: PluginDescriptor,
		id: PluginId,
		audio_buffers: AudioBuffers,
		event_buffers: EventBuffers,
		consumer: Consumer<AudioThreadMessage<Event>>,
	) -> Self {
		Self {
			processor: None,
			descriptor,
			id,
			steady_time: 0,
			audio_buffers,
			event_buffers,
			consumer,
			needs_reset: false,
			realtime: true,
		}
	}

	#[must_use]
	pub fn descriptor(&self) -> &PluginDescriptor {
		&self.descriptor
	}

	#[must_use]
	pub fn id(&self) -> PluginId {
		self.id
	}

	fn recv_events(&mut self, events: &mut Vec<Event>) {
		loop {
			while let Ok(msg) = self.consumer.pop() {
				trace!("{}: {msg:?}", self.descriptor);

				match msg {
					AudioThreadMessage::Activated(processor, latency) => {
						self.processor = Some(processor);
						self.audio_buffers.latency_changed(latency);
					}
					AudioThreadMessage::SetRealtime(realtime) => self.realtime = realtime,
					AudioThreadMessage::Event(event) => events.push(event),
				}
			}

			if let Some(mut processor) = self.processor.take() {
				if let Some(sender) = processor.access_shared_handler(|s| {
					CURRENT_THREAD_ID.with(|&id| s.audio_thread.store(id, Relaxed));
					s.needs_restart.load(Relaxed).then(|| s.sender.clone())
				}) {
					sender
						.send(MainThreadMessage::Restart(NoClone(NoDebug(
							processor.0.into_stopped(),
						))))
						.unwrap();
				} else {
					if self.needs_reset {
						self.needs_reset = false;
						processor.reset();
					}

					self.processor = Some(processor);
				}
			}

			if self.realtime || self.processor.is_some() {
				break;
			}

			spin_loop();
		}
	}

	pub fn process(&mut self, audio: &mut [f32], events: &mut Vec<Event>, mix_level: f32) {
		self.recv_events(events);

		let Some(processor) = &mut self.processor else {
			return;
		};

		if processor.access_shared_handler(|s| !s.processing.load(Relaxed))
			&& events.is_empty()
			&& audio.iter().all(|&f| f == 0.0)
		{
			trace!("{}: skipping process", &self.descriptor);
			self.flush(events);
			return;
		}

		match processor.ensure_processing_started() {
			Ok(started_processor) => {
				self.audio_buffers.read_in(audio);
				self.event_buffers.read_in(events);

				let (input_audio, mut output_audio) = self.audio_buffers.prepare(audio.len() / 2);

				started_processor.access_handler_mut(|at| at.processing = true);

				trace!("{}: processing", &self.descriptor);
				let status = started_processor
					.process(
						&input_audio,
						&mut output_audio,
						&self.event_buffers.input_events.as_input(),
						&mut self.event_buffers.output_events.as_output(),
						Some(self.steady_time),
						None,
					)
					.unwrap();

				started_processor.access_handler_mut(|at| at.processing = false);

				self.steady_time += u64::from(input_audio.min_available_frames_with(&output_audio));

				self.audio_buffers.write_out(audio, mix_level);
				self.event_buffers.write_out(events);

				let processing = match status {
					ProcessStatus::Continue | ProcessStatus::Tail => true,
					ProcessStatus::ContinueIfNotQuiet => audio.iter().any(|&f| f != 0.0),
					ProcessStatus::Sleep => false,
				};

				if !processing {
					processor.ensure_processing_stopped();
				}

				processor.access_shared_handler(|s| {
					s.processing.store(processing, Relaxed);
				});
			}
			Err(err) => {
				warn!("{}: {err}", self.descriptor);
				self.flush(events);
			}
		}
	}

	pub fn flush(&mut self, events: &mut Vec<Event>) {
		self.recv_events(events);

		let Some(processor) = &mut self.processor else {
			return;
		};

		if !processor.access_shared_handler(|s| s.needs_flush.swap(false, Relaxed))
			&& events.is_empty()
		{
			trace!("{}: skipping flush", &self.descriptor);
			return;
		}

		if let Some(&params) = processor.access_shared_handler(|s| s.ext.params.get()) {
			self.event_buffers.read_in(events);

			trace!("{}: flushing events", &self.descriptor);
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
		self.steady_time = 0;
	}

	#[must_use]
	pub fn delay(&self) -> usize {
		self.audio_buffers.delay()
	}
}

impl<Event: EventImpl> Drop for AudioProcessor<Event> {
	fn drop(&mut self) {
		if let Some(mut processor) = self.processor.take() {
			processor.ensure_processing_stopped();
			processor
				.access_shared_handler(|s| s.sender.clone())
				.send(MainThreadMessage::Destroy(NoClone(NoDebug(
					processor.0.into_stopped(),
				))))
				.unwrap();
		}
	}
}
