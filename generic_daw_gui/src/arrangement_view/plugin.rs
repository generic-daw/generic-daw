use generic_daw_core::{
	PluginId,
	clap_host::{self, AudioThread, HostInfo, MainThreadMessage, PluginDescriptor},
};
use std::sync::mpsc::Receiver;

#[derive(Debug)]
pub struct Plugin {
	pub id: PluginId,
	pub descriptor: PluginDescriptor,
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
	) -> (Self, AudioThread, Receiver<MainThreadMessage>) {
		let (core, processor, receiver) = clap_host::Plugin::new(&descriptor, host);
		let gui = Plugin {
			id: PluginId::unique(),
			descriptor,
			mix: 1.0,
		};
		(Self { core, gui }, processor, receiver)
	}
}
