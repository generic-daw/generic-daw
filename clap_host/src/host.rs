use crate::{MainThread, Shared};
use clack_extensions::{audio_ports::HostAudioPorts, gui::HostGui, log::HostLog, timer::HostTimer};
use clack_host::prelude::*;

pub struct Host;

impl HostHandlers for Host {
    type Shared<'a> = Shared;
    type MainThread<'a> = MainThread<'a>;
    type AudioProcessor<'a> = ();

    fn declare_extensions(builder: &mut HostExtensions<'_, Self>, _shared: &Self::Shared<'_>) {
        builder.register::<HostAudioPorts>();
        builder.register::<HostGui>();
        builder.register::<HostLog>();
        builder.register::<HostTimer>();
    }
}
