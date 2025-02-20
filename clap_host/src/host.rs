use crate::{MainThread, Shared};
use clack_extensions::{
    audio_ports::HostAudioPorts, gui::HostGui, log::HostLog, note_ports::HostNotePorts,
    params::HostParams, state::HostState, timer::HostTimer,
};
use clack_host::prelude::*;

#[derive(Clone, Copy)]
pub struct Host;

impl HostHandlers for Host {
    type Shared<'a> = Shared;
    type MainThread<'a> = MainThread;
    type AudioProcessor<'a> = ();

    fn declare_extensions(builder: &mut HostExtensions<'_, Self>, _shared: &Self::Shared<'_>) {
        builder.register::<HostAudioPorts>();
        builder.register::<HostGui>();
        builder.register::<HostLog>();
        builder.register::<HostNotePorts>();
        builder.register::<HostParams>();
        builder.register::<HostState>();
        builder.register::<HostTimer>();
    }
}
