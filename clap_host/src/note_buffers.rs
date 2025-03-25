use clack_extensions::note_ports::{NoteDialect, NotePortInfoBuffer, PluginNotePorts};
use clack_host::{
    events::{
        Match,
        event_types::{MidiEvent, NoteChokeEvent, NoteOffEvent, NoteOnEvent},
    },
    prelude::*,
};

#[derive(Debug, Default)]
pub struct NoteBuffers {
    pub input_events: EventBuffer,
    pub output_events: EventBuffer,

    pub main_input_port: u16,
    pub input_prefers_midi: bool,

    pub main_output_port: u16,
    pub output_prefers_midi: bool,
}

impl NoteBuffers {
    pub(crate) fn new(plugin: &mut PluginMainThreadHandle<'_>) -> Self {
        let (main_input_port, input_prefers_midi) =
            Self::from_ports(plugin, true).unwrap_or_default();
        let (main_output_port, output_prefers_midi) =
            Self::from_ports(plugin, false).unwrap_or_default();

        Self {
            input_events: EventBuffer::new(),
            output_events: EventBuffer::new(),

            main_input_port,
            input_prefers_midi,

            main_output_port,
            output_prefers_midi,
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

    pub fn note_on_event(&mut self, time: u32, channel: u8, key: u8, velocity: f64) {
        if self.input_prefers_midi {
            self.input_events.push(&MidiEvent::new(
                time,
                self.main_input_port,
                [0x90 | channel, key, (velocity * 127.0) as u8],
            ));
        } else {
            self.input_events.push(&NoteOnEvent::new(
                time,
                Pckn::new(self.main_input_port, channel, key, Match::All),
                velocity,
            ));
        }
    }

    pub fn note_off_event(&mut self, time: u32, channel: u8, key: u8, velocity: f64) {
        if self.input_prefers_midi {
            self.input_events.push(&MidiEvent::new(
                time,
                self.main_input_port,
                [0x80 | channel, key, (velocity * 127.0) as u8],
            ));
        } else {
            self.input_events.push(&NoteOffEvent::new(
                time,
                Pckn::new(self.main_input_port, channel, key, Match::All),
                velocity,
            ));
        }
    }

    pub fn all_notes_off(&mut self, time: u32, channel: u8) {
        if self.input_prefers_midi {
            self.input_events.push(&MidiEvent::new(
                time,
                self.main_input_port,
                [0xb0 | channel, 0x7b, 0x00],
            ));
        } else {
            self.input_events.push(&NoteChokeEvent::new(
                time,
                Pckn::new(self.main_input_port, channel, Match::All, Match::All),
            ));
        }
    }
}
