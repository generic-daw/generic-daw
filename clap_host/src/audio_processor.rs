use crate::{audio_buffers::AudioBuffers, event_buffers::EventBuffers, shared::Shared};
use clack_host::host::AudioProcessorHandler;
use std::sync::atomic::Ordering::Relaxed;

#[derive(Debug)]
pub struct AudioProcessor<'a> {
	pub shared: &'a Shared<'a>,
	pub audio_buffers: Option<AudioBuffers>,
	pub event_buffers: Option<EventBuffers>,
}

impl<'a> AudioProcessor<'a> {
	pub fn new(
		shared: &'a Shared<'a>,
		audio_buffers: AudioBuffers,
		event_buffers: EventBuffers,
	) -> Self {
		shared.needs_activate.store(false, Relaxed);

		Self {
			shared,
			audio_buffers: Some(audio_buffers),
			event_buffers: Some(event_buffers),
		}
	}
}

impl<'a> AudioProcessorHandler<'a> for AudioProcessor<'a> {}

impl Drop for AudioProcessor<'_> {
	fn drop(&mut self) {
		self.shared.needs_deactivate.store(false, Relaxed);
	}
}
