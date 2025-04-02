use crate::Event;
use clack_extensions::note_ports::{NoteDialect, NotePortInfoBuffer, PluginNotePorts};
use clack_host::prelude::*;

#[derive(Debug, Default)]
pub struct EventBuffers {
    pub input_events: EventBuffer,
    pub output_events: EventBuffer,

    pub main_input_port: u16,
    pub input_prefers_midi: bool,
}

impl EventBuffers {
    pub fn new(plugin: &mut PluginMainThreadHandle<'_>) -> Self {
        let (main_input_port, input_prefers_midi) =
            Self::from_ports(plugin, true).unwrap_or_default();

        Self {
            input_events: EventBuffer::new(),
            output_events: EventBuffer::new(),

            main_input_port,
            input_prefers_midi,
        }
    }

    fn from_ports(plugin: &mut PluginMainThreadHandle<'_>, is_input: bool) -> Option<(u16, bool)> {
        let ports = plugin.get_extension::<PluginNotePorts>()?;

        let mut buffer = NotePortInfoBuffer::new();

        (0..ports.count(plugin, is_input).min(u32::from(u16::MAX))).find_map(|i| {
            let port = ports.get(plugin, i, is_input, &mut buffer)?;

            (port.supported_dialects.supports(NoteDialect::Midi)
                || port.supported_dialects.supports(NoteDialect::Clap))
            .then_some((
                i as u16,
                port.preferred_dialect
                    .is_some_and(|d| d == NoteDialect::Midi),
            ))
        })
    }

    pub fn read_in(&mut self, events: &mut Vec<Event>) {
        self.input_events.clear();
        if self.input_prefers_midi {
            for e in events {
                e.push_as_midi(self.main_input_port, &mut self.input_events);
            }
        } else {
            for e in events {
                e.push_as_clap(self.main_input_port, &mut self.input_events);
            }
        }
        self.input_events.sort();
    }

    pub fn write_out(&mut self, events: &mut Vec<Event>) {
        self.output_events.sort();
        events.extend(
            self.output_events
                .iter()
                .filter_map(|e| Event::try_from(e).ok()),
        );
        self.output_events.clear();
    }

    pub fn reset(&mut self) {
        if self.input_prefers_midi {
            Event::AllOff {
                time: 0,
                channel: 0,
            }
            .push_as_midi(self.main_input_port, &mut self.input_events);
        } else {
            Event::AllOff {
                time: 0,
                channel: 0,
            }
            .push_as_clap(self.main_input_port, &mut self.input_events);
        }
    }
}
