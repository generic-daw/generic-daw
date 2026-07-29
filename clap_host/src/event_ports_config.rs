use crate::host::Host;
use clack_extensions::note_ports::{NoteDialect, NotePortInfoBuffer};
use clack_host::prelude::*;

#[derive(Debug, Default)]
pub struct EventPortsConfig {
	pub preferred_dialects: Box<[NoteDialect]>,
}

impl EventPortsConfig {
	pub fn from_ports(plugin: &mut PluginInstance<Host>, is_input: bool) -> Option<Self> {
		let note_ports = *plugin.access_shared_handler(|s| s.ext.note_ports.get())?;

		let mut buffer = NotePortInfoBuffer::new();
		let preferred_dialects = (0..note_ports.count(&mut plugin.plugin_handle(), is_input))
			.map(|i| {
				note_ports
					.get(&mut plugin.plugin_handle(), i, is_input, &mut buffer)
					.and_then(|info| info.preferred_dialect)
					.unwrap_or(NoteDialect::Clap)
			})
			.collect::<Box<_>>();

		Some(Self { preferred_dialects })
	}
}
