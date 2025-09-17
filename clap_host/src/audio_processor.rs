use crate::{
	EventImpl, MainThreadMessage, PluginDescriptor, PluginId, audio_buffers::AudioBuffers,
	event_buffers::EventBuffers, host::Host, shared::CURRENT_THREAD_ID,
};
use clack_host::process::PluginAudioProcessor;
use generic_daw_utils::{NoClone, NoDebug};
use log::{trace, warn};
use rtrb::Consumer;
use std::sync::atomic::Ordering::Relaxed;

#[derive(Debug)]
pub enum AudioThreadMessage<Event: EventImpl> {
	Activated(NoDebug<PluginAudioProcessor<Host>>),
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
	receiver: Consumer<AudioThreadMessage<Event>>,
}

impl<Event: EventImpl> AudioProcessor<Event> {
	#[must_use]
	pub fn new(
		descriptor: PluginDescriptor,
		id: PluginId,
		audio_buffers: AudioBuffers,
		event_buffers: EventBuffers,
		receiver: Consumer<AudioThreadMessage<Event>>,
	) -> Self {
		Self {
			processor: None,
			descriptor,
			id,
			steady_time: 0,
			audio_buffers,
			event_buffers,
			receiver,
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
		while let Ok(msg) = self.receiver.pop() {
			trace!("{}: {msg:?}", self.descriptor);

			match msg {
				AudioThreadMessage::Activated(processor) => self.processor = Some(processor),
				AudioThreadMessage::Event(event) => events.push(event),
			}
		}

		if let Some(processor) = self.processor.take() {
			if processor.access_shared_handler(|s| {
				CURRENT_THREAD_ID.with(|&id| s.audio_thread.store(id, Relaxed));
				self.audio_buffers.latency_changed(s.latency.load(Relaxed));
				s.needs_restart.load(Relaxed)
			}) {
				processor
					.access_shared_handler(|s| s.sender.clone())
					.send(MainThreadMessage::Restart(NoClone(processor)))
					.unwrap();
			} else {
				self.processor = Some(processor);
			}
		}
	}

	pub fn process(&mut self, audio: &mut [f32], events: &mut Vec<Event>, mix_level: f32) {
		self.recv_events(events);

		let Some(processor) = &mut self.processor else {
			return;
		};

		match processor.ensure_processing_started() {
			Ok(processor) => {
				self.audio_buffers.read_in(audio);
				self.event_buffers.read_in(events);

				let (input_audio, mut output_audio) = self.audio_buffers.prepare(audio.len() / 2);

				processor.access_handler(|at| at.processing.store(true, Relaxed));

				processor
					.process(
						&input_audio,
						&mut output_audio,
						&self.event_buffers.input_events.as_input(),
						&mut self.event_buffers.output_events.as_output(),
						Some(self.steady_time),
						None,
					)
					.unwrap();

				processor.access_handler(|at| at.processing.store(false, Relaxed));

				self.steady_time += u64::from(input_audio.min_available_frames_with(&output_audio));

				self.audio_buffers.write_out(audio, mix_level);
				self.event_buffers.write_out(events);
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

		self.event_buffers.read_in(events);

		if let Some(&params) = processor.access_shared_handler(|s| s.ext.params.get()) {
			params.flush_active(
				&mut processor.plugin_handle(),
				&self.event_buffers.input_events.as_input(),
				&mut self.event_buffers.output_events.as_output(),
			);
		}

		self.event_buffers.write_out(events);
		events.clear();
	}

	pub fn reset(&mut self) {
		if let Some(processor) = &mut self.processor {
			processor.reset();
		}
		self.steady_time = 0;
		self.event_buffers.reset();
	}

	#[must_use]
	pub fn delay(&self) -> usize {
		self.processor
			.as_ref()
			.map_or(self.audio_buffers.delay(), |processor| {
				processor.access_shared_handler(|s| s.latency.load(Relaxed)) as usize
			})
	}
}

impl<Event: EventImpl> Drop for AudioProcessor<Event> {
	fn drop(&mut self) {
		if let Some(processor) = self.processor.take() {
			processor
				.access_shared_handler(|s| s.sender.clone())
				.send(MainThreadMessage::Destroy(NoClone(processor)))
				.unwrap();
		}
	}
}
