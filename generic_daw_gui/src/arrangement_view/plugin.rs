use generic_daw_core::{
	PluginId, PushSlot,
	clap_host::{self, HostInfo, MainThreadMessage, PluginDescriptor},
};
use std::sync::mpsc::Receiver;

#[derive(Debug)]
pub struct Plugin {
	pub id: PluginId,
	pub descriptor: PluginDescriptor,
	pub active: bool,
	pub mix: f32,
	pub s1: oneshot::Sender<PushSlot<generic_daw_core::Plugin>>,
	pub s2: oneshot::Sender<PushSlot<PushSlot<generic_daw_core::Plugin>>>,
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
		PushSlot<PushSlot<generic_daw_core::Plugin>>,
		Receiver<MainThreadMessage>,
	)> {
		let (core, receiver) = clap_host::Plugin::new(&descriptor, host)?;
		let (s1, r1) = oneshot::channel();
		let (s2, r2) = oneshot::channel();
		let gui = Plugin {
			id: PluginId::unique(),
			descriptor,
			active: false,
			mix: 1.0,
			s1,
			s2,
		};
		Some((
			Self { core, gui },
			PushSlot::new(Some(PushSlot::new(None, r1)), r2),
			receiver,
		))
	}
}
