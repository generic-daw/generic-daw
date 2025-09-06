use crate::{EventImpl, MainThreadMessage, shared::Shared};
use clack_extensions::note_ports::{NoteDialect, NotePortInfoBuffer, PluginNotePorts};
use clack_host::{
	events::{
		EventFlags, Match,
		event_types::{MidiEvent, NoteChokeEvent},
	},
	prelude::*,
};

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

		(0..ports.count(plugin, is_input).min(u16::MAX.into())).find_map(|i| {
			let port = ports.get(plugin, i, is_input, &mut buffer)?;

			(port.supported_dialects.supports(NoteDialect::Midi)
				|| port.supported_dialects.supports(NoteDialect::Clap))
			.then_some((i as u16, port.preferred_dialect == Some(NoteDialect::Midi)))
		})
	}

	pub fn read_in(&mut self, events: &mut Vec<impl EventImpl>) {
		self.input_events.clear();

		if self.input_prefers_midi {
			for e in events.drain(..) {
				self.input_events.push(&e.to_midi(self.main_input_port));
			}
		} else {
			for e in events.drain(..) {
				self.input_events.push(&e.to_clap(self.main_input_port));
			}
		}

		self.input_events.sort();
	}

	pub fn write_out<Event: EventImpl>(&mut self, events: &mut Vec<Event>, shared: &Shared<Event>) {
		for unknown in &self.output_events {
			let Some(event) = Event::try_from_unknown(unknown) else {
				continue;
			};

			if unknown.header().flags().contains(EventFlags::IS_LIVE) {
				shared
					.sender
					.try_send(MainThreadMessage::LiveEvent(event))
					.unwrap();
			} else {
				events.push(event);
			}
		}

		self.output_events.clear();
	}

	pub fn reset(&mut self) {
		if self.input_prefers_midi {
			self.input_events
				.push(&MidiEvent::new(0, self.main_input_port, [0xb0, 0x7b, 0x00]));
		} else {
			self.input_events.push(&NoteChokeEvent::new(
				0,
				Pckn::new(self.main_input_port, 0u8, Match::All, Match::All),
			));
		}
	}
}
