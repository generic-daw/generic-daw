use crate::{EventImpl, MainThread, Shared};
use clack_extensions::{
	audio_ports::HostAudioPorts, gui::HostGui, latency::HostLatency, log::HostLog,
	note_ports::HostNotePorts, params::HostParams, state::HostState, timer::HostTimer,
};
use clack_host::prelude::*;
use std::marker::PhantomData;

#[derive(Clone, Copy, Debug)]
pub struct Host<Event: EventImpl>(PhantomData<Event>);

impl<Event: EventImpl> HostHandlers for Host<Event> {
	type Shared<'a> = Shared<Event>;
	type MainThread<'a> = MainThread<'a, Event>;
	type AudioProcessor<'a> = ();

	fn declare_extensions(builder: &mut HostExtensions<'_, Self>, _shared: &Self::Shared<'_>) {
		builder.register::<HostAudioPorts>();
		builder.register::<HostGui>();
		builder.register::<HostLatency>();
		builder.register::<HostLog>();
		builder.register::<HostNotePorts>();
		builder.register::<HostParams>();
		builder.register::<HostState>();
		builder.register::<HostTimer>();
	}
}
