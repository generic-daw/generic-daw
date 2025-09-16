use crate::host::Host;
use clack_extensions::audio_ports::{AudioPortFlags, AudioPortInfoBuffer};
use clack_host::prelude::*;

#[derive(Debug, Default)]
pub struct AudioPortsConfig {
	pub port_channel_counts: Box<[usize]>,
	pub main_port_index: usize,
}

impl AudioPortsConfig {
	pub fn from_ports(plugin: &mut PluginInstance<Host>, is_input: bool) -> Option<Self> {
		let ports = *plugin.access_shared_handler(|s| s.ext.audio_ports.get())?;

		let mut buffer = AudioPortInfoBuffer::new();
		let mut main_port_index = None;
		let mut port_channel_counts = Vec::new();

		for i in 0..ports.count(&mut plugin.plugin_handle(), is_input) {
			let Some(info) = ports.get(&mut plugin.plugin_handle(), i, is_input, &mut buffer)
			else {
				continue;
			};

			if info.flags.contains(AudioPortFlags::IS_MAIN) {
				main_port_index.get_or_insert(i);
			}

			port_channel_counts.push(info.channel_count as usize);
		}

		let port_channel_counts = port_channel_counts.into_boxed_slice();

		let main_port_index = main_port_index
			.map(|i| i as usize)
			.or_else(|| port_channel_counts.iter().position(|&p| p == 2))
			.or_else(|| port_channel_counts.iter().position(|&p| p == 1))
			.unwrap_or_default();

		Some(Self {
			port_channel_counts,
			main_port_index,
		})
	}
}

impl From<&AudioPortsConfig> for AudioPorts {
	fn from(value: &AudioPortsConfig) -> Self {
		Self::with_capacity(
			value.port_channel_counts.iter().sum(),
			value.port_channel_counts.len(),
		)
	}
}
