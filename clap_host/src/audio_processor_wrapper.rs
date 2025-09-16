use crate::host::Host;
use clack_host::process::{PluginAudioProcessor, StoppedPluginAudioProcessor};
use generic_daw_utils::NoDebug;

#[derive(Debug)]
pub struct AudioProcessorWrapper(NoDebug<Option<PluginAudioProcessor<Host>>>);

impl AudioProcessorWrapper {
	pub fn inner(&self) -> &PluginAudioProcessor<Host> {
		(*self.0).as_ref().unwrap()
	}

	pub fn inner_mut(&mut self) -> &mut PluginAudioProcessor<Host> {
		(*self.0).as_mut().unwrap()
	}

	pub fn into_stopped(mut self) -> StoppedPluginAudioProcessor<Host> {
		self.0.take().unwrap().into_stopped()
	}
}

impl From<StoppedPluginAudioProcessor<Host>> for AudioProcessorWrapper {
	fn from(value: StoppedPluginAudioProcessor<Host>) -> Self {
		Self(NoDebug(Some(value.into())))
	}
}

impl Drop for AudioProcessorWrapper {
	fn drop(&mut self) {
		if let Some(inner) = &mut self.0.0 {
			inner.ensure_processing_stopped();
			inner.access_shared_handler(|s| {
				s.once.call_once(|| ());
			});
		}
	}
}
