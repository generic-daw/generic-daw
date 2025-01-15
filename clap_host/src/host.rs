use super::{MainThread, Shared};
use clack_extensions::{
    audio_ports::HostAudioPorts, gui::HostGui, note_ports::HostNotePorts, params::HostParams,
    state::HostState, timer::HostTimer,
};
use clack_host::prelude::*;

#[derive(Debug)]
pub struct Host;

#[derive(Debug)]
pub enum HostThreadMessage {
    ProcessAudio(Vec<Vec<f32>>, EventBuffer),
    State(Vec<u8>),
}

impl HostHandlers for Host {
    type Shared<'a> = Shared;
    type MainThread<'a> = MainThread<'a>;
    type AudioProcessor<'a> = ();

    fn declare_extensions(builder: &mut HostExtensions<'_, Self>, _shared: &Self::Shared<'_>) {
        builder.register::<HostAudioPorts>();
        builder.register::<HostGui>();
        builder.register::<HostNotePorts>();
        builder.register::<HostParams>();
        builder.register::<HostState>();
        builder.register::<HostTimer>();
    }
}
