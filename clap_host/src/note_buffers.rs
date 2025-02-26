use clack_extensions::note_ports::{NoteDialects, NotePortInfoBuffer, PluginNotePorts};
use clack_host::prelude::*;

#[derive(Debug, Default)]
pub struct NoteBuffers {
    pub input_events: EventBuffer,
    pub output_events: EventBuffer,

    pub main_input_port: u16,
    pub main_output_port: u16,
}

impl NoteBuffers {
    pub fn new(plugin: &mut PluginMainThreadHandle<'_>) -> Self {
        Self {
            input_events: EventBuffer::with_capacity(255),
            output_events: EventBuffer::with_capacity(255),

            main_input_port: Self::from_ports(plugin, true).unwrap_or_default(),
            main_output_port: Self::from_ports(plugin, false).unwrap_or_default(),
        }
    }

    fn from_ports(plugin: &mut PluginMainThreadHandle<'_>, is_input: bool) -> Option<u16> {
        let ports = plugin.get_extension::<PluginNotePorts>()?;

        let mut buffer = NotePortInfoBuffer::new();
        let mut main_port_index = None;

        for i in 0..ports.count(plugin, is_input).min(u32::from(u16::MAX)) {
            let Some(info) = ports.get(plugin, i, is_input, &mut buffer) else {
                continue;
            };

            if info.supported_dialects.intersects(NoteDialects::CLAP) {
                main_port_index.get_or_insert(i as u16);
            }
        }

        main_port_index
    }
}
