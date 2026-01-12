use crate::{audio_thread::AudioThread, main_thread::MainThread, shared::Shared};
#[cfg(unix)]
use clack_extensions::posix_fd::HostPosixFd;
use clack_extensions::{
	audio_ports::HostAudioPorts, gui::HostGui, latency::HostLatency, log::HostLog,
	note_ports::HostNotePorts, params::HostParams, preset_discovery::HostPresetLoad,
	state::HostState, thread_check::HostThreadCheck, thread_pool::HostThreadPool, timer::HostTimer,
};
use clack_host::prelude::*;

#[derive(Clone, Copy, Debug)]
pub struct Host;

impl HostHandlers for Host {
	type Shared<'a> = Shared<'a>;
	type MainThread<'a> = MainThread<'a>;
	type AudioProcessor<'a> = AudioThread<'a>;

	fn declare_extensions(builder: &mut HostExtensions<'_, Self>, _shared: &Self::Shared<'_>) {
		builder
			.register::<HostAudioPorts>()
			.register::<HostGui>()
			.register::<HostLatency>()
			.register::<HostLog>()
			.register::<HostNotePorts>()
			.register::<HostParams>()
			.register::<HostPresetLoad>()
			.register::<HostState>()
			.register::<HostThreadCheck>()
			.register::<HostThreadPool>()
			.register::<HostTimer>();

		#[cfg(unix)]
		builder.register::<HostPosixFd>();
	}
}
