#![expect(dead_code)]

use fragile::Fragile;
use generic_daw_core::clap_host::{ClapPluginGui, HostAudioProcessor, PluginAudioProcessor};
use iced::window::Id;
use std::fmt::{Debug, Formatter};

pub struct Opened {
    pub id: Id,
    pub gui: Fragile<ClapPluginGui>,
    pub hap: HostAudioProcessor,
    pub pap: PluginAudioProcessor,
}

impl Debug for Opened {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Opened")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}
