use generic_daw_core::{
	PluginId,
	clap_host::{self, HostInfo, MainThreadMessage, PluginDescriptor},
};
use std::sync::mpsc::Receiver;

#[derive(Debug)]
pub struct Plugin {
	pub id: PluginId,
	pub descriptor: PluginDescriptor,
	pub active: bool,
	pub mix: f32,
}

pub struct PluginPair {
	pub core: clap_host::Plugin,
	pub gui: Plugin,
}

impl PluginPair {
	pub fn new(
		descriptor: PluginDescriptor,
		host: HostInfo,
	) -> Option<(Self, Receiver<MainThreadMessage>)> {
		let (core, receiver) = clap_host::Plugin::new(&descriptor, host)?;
		let gui = Plugin {
			id: PluginId::unique(),
			descriptor,
			active: false,
			mix: 1.0,
		};
		Some((Self { core, gui }, receiver))
	}
}
