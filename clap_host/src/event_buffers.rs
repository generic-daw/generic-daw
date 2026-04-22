use crate::{EventImpl, host::Host};
use clack_extensions::note_ports::{NoteDialect, NotePortInfoBuffer};
use clack_host::prelude::*;

#[derive(Debug, Default)]
pub struct EventBuffers {
	input_events: EventBuffer,
	output_events: EventBuffer,

	main_input_port: u16,
	input_prefers_midi: bool,
}

impl EventBuffers {
	pub fn new(plugin: &mut PluginInstance<Host>) -> Self {
		let (main_input_port, input_prefers_midi) =
			Self::from_ports(plugin, true).unwrap_or_default();

		Self {
			input_events: EventBuffer::new(),
			output_events: EventBuffer::new(),

			main_input_port,
			input_prefers_midi,
		}
	}

	fn from_ports(plugin: &mut PluginInstance<Host>, is_input: bool) -> Option<(u16, bool)> {
		let note_ports = *plugin.access_shared_handler(|s| s.ext.note_ports.get())?;

		let mut buffer = NotePortInfoBuffer::new();

		(0..note_ports.count(&mut plugin.plugin_handle(), is_input)).find_map(|i| {
			let port = note_ports.get(&mut plugin.plugin_handle(), i, is_input, &mut buffer)?;

			(port.supported_dialects.supports(NoteDialect::Clap)
				|| port.supported_dialects.supports(NoteDialect::Midi))
			.then_some((
				i as u16,
				port.preferred_dialect == Some(NoteDialect::Midi)
					|| !port.supported_dialects.supports(NoteDialect::Clap),
			))
		})
	}

	pub fn read_in(
		&mut self,
		events: &mut Vec<impl EventImpl>,
	) -> (InputEvents<'_>, OutputEvents<'_>) {
		for event in events.drain(..) {
			self.push(event);
		}

		self.input_events.sort();

		(self.input_events.as_input(), self.output_events.as_output())
	}

	pub fn push(&mut self, event: impl EventImpl) {
		if self.input_prefers_midi {
			self.input_events.push(&event.to_midi(self.main_input_port));
		} else {
			self.input_events.push(&event.to_clap(self.main_input_port));
		}
	}

	pub fn write_out(&mut self, events: &mut Vec<impl EventImpl>) {
		events.extend(
			self.output_events
				.iter()
				.filter_map(EventImpl::try_from_unknown),
		);

		self.input_events.clear();
		self.output_events.clear();
	}
}
