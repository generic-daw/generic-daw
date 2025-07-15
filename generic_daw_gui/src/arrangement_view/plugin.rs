use generic_daw_core::clap_host::{PluginDescriptor, PluginId};

#[derive(Debug)]
pub struct Plugin {
	pub id: PluginId,
	pub descriptor: PluginDescriptor,
	pub enabled: bool,
	pub mix: f32,
}

impl Plugin {
	pub fn new(id: PluginId, descriptor: PluginDescriptor) -> Self {
		Self {
			id,
			descriptor,
			enabled: true,
			mix: 1.0,
		}
	}
}
