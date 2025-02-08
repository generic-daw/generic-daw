use fragile::Fragile;
use generic_daw_core::clap_host::{ClapPluginGui, HostAudioProcessor, PluginAudioProcessor};
use iced::window::Id;
use std::fmt::{Debug, Formatter};

pub struct Opened {
    pub id: Id,
    pub gui: Fragile<ClapPluginGui>,
    pub host_audio_processor: HostAudioProcessor,
    pub plugin_audio_processor: PluginAudioProcessor,
}

impl Debug for Opened {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Opened")
            .field("id", &self.id)
            .field("host_audio_processor", &self.host_audio_processor)
            .field("plugin_audio_processor", &self.plugin_audio_processor)
            .finish_non_exhaustive()
    }
}
