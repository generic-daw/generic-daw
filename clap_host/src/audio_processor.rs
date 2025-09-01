use crate::{
	EventImpl, Host, PluginDescriptor, PluginId, audio_buffers::AudioBuffers,
	event_buffers::EventBuffers,
};
use async_channel::Receiver;
use clack_host::process::StartedPluginAudioProcessor;
use generic_daw_utils::NoDebug;
use log::trace;

#[derive(Clone, Copy, Debug)]
pub enum AudioThreadMessage<Event: EventImpl> {
	RequestRestart,
	LatencyChanged(u32),
	Event(Event),
}

#[derive(Debug)]
pub struct AudioProcessor<Event: EventImpl> {
	started_processor: Option<NoDebug<StartedPluginAudioProcessor<Host<Event>>>>,
	descriptor: PluginDescriptor,
	id: PluginId,
	steady_time: u64,
	audio_buffers: AudioBuffers,
	event_buffers: EventBuffers,
	receiver: Receiver<AudioThreadMessage<Event>>,
}

impl<Event: EventImpl> AudioProcessor<Event> {
	#[must_use]
	pub fn new(
		started_processor: StartedPluginAudioProcessor<Host<Event>>,
		descriptor: PluginDescriptor,
		id: PluginId,
		audio_buffers: AudioBuffers,
		event_buffers: EventBuffers,
		receiver: Receiver<AudioThreadMessage<Event>>,
	) -> Self {
		Self {
			started_processor: Some(started_processor.into()),
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

	pub fn process(&mut self, audio: &mut [f32], events: &mut Vec<Event>, mix_level: f32) {
		while let Ok(msg) = self.receiver.try_recv() {
			trace!("{}: {msg:?}", self.descriptor);

			match msg {
				AudioThreadMessage::RequestRestart => {
					let mut stopped_processor =
						self.started_processor.take().unwrap().0.stop_processing();

					let started_processor = loop {
						match stopped_processor.start_processing() {
							Ok(started_processor) => break started_processor,
							Err(err) => stopped_processor = err.into_stopped_processor(),
						}
					};

					self.started_processor = Some(started_processor.into());
				}
				AudioThreadMessage::LatencyChanged(latency) => {
					self.audio_buffers.latency_changed(latency);
				}
				AudioThreadMessage::Event(event) => events.push(event),
			}
		}

		self.audio_buffers.read_in(audio);
		self.event_buffers.read_in(events);

		let (input_audio, mut output_audio) = self.audio_buffers.prepare(audio.len());

		self.started_processor
			.as_mut()
			.unwrap()
			.process(
				&input_audio,
				&mut output_audio,
				&self.event_buffers.input_events.as_input(),
				&mut self.event_buffers.output_events.as_output(),
				Some(self.steady_time),
				None,
			)
			.unwrap();

		self.steady_time += u64::from(input_audio.min_available_frames_with(&output_audio));

		self.audio_buffers.write_out(audio, mix_level);
		self.event_buffers.write_out(
			events,
			self.started_processor
				.as_ref()
				.unwrap()
				.access_shared_handler(|s| s),
		);
	}

	pub fn reset(&mut self) {
		self.event_buffers.reset();
		self.started_processor.as_mut().unwrap().reset();
		self.steady_time = 0;
	}

	#[must_use]
	pub fn delay(&self) -> usize {
		self.audio_buffers.delay()
	}
}
