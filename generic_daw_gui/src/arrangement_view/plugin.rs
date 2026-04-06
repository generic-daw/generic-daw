use generic_daw_core::{
	Event, PluginId, Transport,
	clap_host::{self, AudioProcessor, HostInfo, MainThreadMessage, PluginDescriptor},
};
use std::sync::mpsc::Receiver;

#[derive(Debug)]
pub struct Plugin {
	pub id: PluginId,
	pub descriptor: PluginDescriptor,
	pub enabled: bool,
	pub mix: f32,
}

pub struct PluginPair {
	pub core: clap_host::Plugin<Event>,
	pub gui: Plugin,
}

impl PluginPair {
	pub fn new(
		descriptor: PluginDescriptor,
		transport: &Transport,
		host: &HostInfo,
	) -> (Self, AudioProcessor<Event>, Receiver<MainThreadMessage>) {
		let (core, processor, receiver) = clap_host::Plugin::new(
			descriptor.clone(),
			transport.sample_rate,
			transport.frames,
			host,
		);
		let gui = Plugin {
			id: PluginId::unique(),
			descriptor,
			enabled: true,
			mix: 1.0,
		};
		(Self { core, gui }, processor, receiver)
	}
}
