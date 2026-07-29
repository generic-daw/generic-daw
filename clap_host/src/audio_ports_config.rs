use crate::host::Host;
use clack_extensions::audio_ports::AudioPortInfoBuffer;
use clack_host::prelude::*;

#[derive(Debug, Default)]
pub struct AudioPortsConfig {
	pub channel_counts: Box<[u32]>,
}

impl AudioPortsConfig {
	pub fn from_ports(plugin: &mut PluginInstance<Host>, is_input: bool) -> Option<Self> {
		let audio_ports = *plugin.access_shared_handler(|s| s.ext.audio_ports.get())?;

		let mut buffer = AudioPortInfoBuffer::new();
		let channel_counts = (0..audio_ports.count(&mut plugin.plugin_handle(), is_input))
			.map(|i| {
				audio_ports
					.get(&mut plugin.plugin_handle(), i, is_input, &mut buffer)
					.map_or(0, |info| info.channel_count)
			})
			.collect::<Box<_>>();

		Some(Self { channel_counts })
	}
}
