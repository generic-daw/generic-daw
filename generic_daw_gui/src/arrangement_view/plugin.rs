use generic_daw_core::{PluginId, clap_host::PluginDescriptor};

#[derive(Debug)]
pub struct Plugin {
	pub id: PluginId,
	pub descriptor: PluginDescriptor,
	pub enabled: bool,
	pub mix: f32,
}

impl Plugin {
	pub fn new(descriptor: PluginDescriptor) -> Self {
		Self {
			id: PluginId::unique(),
			descriptor,
			enabled: true,
			mix: 1.0,
		}
	}
}
