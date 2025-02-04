use generic_daw_core::clap_host::{ClapPluginGuiWrapper, HostAudioProcessor, PluginAudioProcessor};
use iced::window::Id;
use std::fmt::{Debug, Formatter};

pub struct Opened {
    pub id: Id,
    pub gui: ClapPluginGuiWrapper,
    pub host_audio_processor: HostAudioProcessor,
    pub plugin_audio_processor: PluginAudioProcessor,
}

impl Debug for Opened {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenedMessage")
            .field("id", &self.id)
            .field("host_audio_processor", &self.host_audio_processor)
            .field("plugin_audio_processor", &self.plugin_audio_processor)
            .finish_non_exhaustive()
    }
}
