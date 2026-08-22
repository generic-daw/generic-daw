use generic_daw_core::{
	PluginId, PushSlot,
	clap_host::{self, AudioThread, HostInfo, MainThreadMessage, PluginDescriptor},
};
use std::sync::mpsc::Receiver;

#[derive(Debug)]
pub struct Plugin {
	pub id: PluginId,
	pub descriptor: PluginDescriptor,
	pub active: bool,
	pub mix: f32,
	pub s: oneshot::Sender<PushSlot<Option<AudioThread>>>,
}

pub struct PluginPair {
	pub core: clap_host::Plugin,
	pub gui: Plugin,
}

impl PluginPair {
	pub fn new(
		descriptor: PluginDescriptor,
		host: HostInfo,
	) -> Option<(
		Self,
		PushSlot<Option<AudioThread>>,
		Receiver<MainThreadMessage>,
	)> {
		let (core, receiver) = clap_host::Plugin::new(&descriptor, host)?;
		let (s, r) = oneshot::channel();
		let gui = Plugin {
			id: PluginId::unique(),
			descriptor,
			active: false,
			mix: 1.0,
			s,
		};
		Some((Self { core, gui }, PushSlot::new(None, r), receiver))
	}
}
