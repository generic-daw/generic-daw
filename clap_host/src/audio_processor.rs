use crate::{
	EventImpl, Host, PluginDescriptor, PluginId, audio_buffers::AudioBuffers,
	event_buffers::EventBuffers, shared::CURRENT_THREAD_ID,
};
use clack_host::process::PluginAudioProcessor;
use generic_daw_utils::NoDebug;
use log::{trace, warn};
use rtrb::Consumer;
use std::sync::atomic::Ordering::{Relaxed, Release};

#[derive(Clone, Copy, Debug)]
pub enum AudioThreadMessage<Event: EventImpl> {
	RequestRestart,
	LatencyChanged(u32),
	Event(Event),
}

#[derive(Debug)]
pub struct AudioProcessor<Event: EventImpl> {
	processor: NoDebug<PluginAudioProcessor<Host>>,
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
		started_processor: impl Into<PluginAudioProcessor<Host>>,
		descriptor: PluginDescriptor,
		id: PluginId,
		audio_buffers: AudioBuffers,
		event_buffers: EventBuffers,
		receiver: Consumer<AudioThreadMessage<Event>>,
	) -> Self {
		Self {
			processor: started_processor.into().into(),
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
				AudioThreadMessage::RequestRestart => _ = self.processor.stop_processing(),
				AudioThreadMessage::LatencyChanged(latency) => {
					self.audio_buffers.latency_changed(latency);
				}
				AudioThreadMessage::Event(event) => events.push(event),
			}
		}
	}

	pub fn process(&mut self, audio: &mut [f32], events: &mut Vec<Event>, mix_level: f32) {
		self.recv_events(events);

		match self.processor.ensure_processing_started() {
			Ok(processor) => {
				self.audio_buffers.read_in(audio);
				self.event_buffers.read_in(events);

				let (input_audio, mut output_audio) = self.audio_buffers.prepare(audio.len());

				processor.access_shared_handler(|s| {
					CURRENT_THREAD_ID.with(|&id| s.audio_thread.store(id, Relaxed));
				});
				processor.access_handler(|at| at.processing.store(true, Release));

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

				processor.access_handler(|at| at.processing.store(false, Release));

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

		self.event_buffers.read_in(events);

		if let Some(&params) = self.processor.access_shared_handler(|s| s.params.get()) {
			params.flush_active(
				&mut self.processor.plugin_handle(),
				&self.event_buffers.input_events.as_input(),
				&mut self.event_buffers.output_events.as_output(),
			);
		}

		self.event_buffers.write_out(events);
		events.clear();
	}

	pub fn reset(&mut self) {
		self.event_buffers.reset();
		self.processor.reset();
		self.steady_time = 0;
	}

	#[must_use]
	pub fn delay(&self) -> usize {
		self.audio_buffers.delay()
	}
}
