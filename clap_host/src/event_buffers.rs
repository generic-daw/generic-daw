use crate::{EventImpl, host::Host, param::Param};
use clack_extensions::{
	note_ports::{NoteDialect, NotePortInfoBuffer},
	params::ParamInfoFlags,
};
use clack_host::prelude::*;

#[derive(Debug, Default)]
pub struct EventBuffers {
	input_events: EventBuffer,
	output_events: EventBuffer,

	main_input_port: u16,
	input_prefers_midi: bool,
}

impl EventBuffers {
	pub fn new(plugin: &mut PluginInstance<Host>, params: &[Param]) -> Self {
		let input_ports = Self::from_ports(plugin, true);
		let (main_input_port, input_prefers_midi) = input_ports.unwrap_or_default();

		let event_buffers_cap = params
			.iter()
			.filter(|param| {
				!param
					.flags
					.intersects(ParamInfoFlags::IS_HIDDEN | ParamInfoFlags::IS_READONLY)
			})
			.count() + 128;

		Self {
			input_events: EventBuffer::with_capacity(event_buffers_cap),
			output_events: EventBuffer::with_capacity(event_buffers_cap),

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

	pub fn are_inputs_empty(&self) -> bool {
		self.input_events.is_empty()
	}

	pub fn are_outputs_empty(&self) -> bool {
		self.output_events.is_empty()
	}

	pub fn push(&mut self, event: impl EventImpl) {
		self.input_events
			.push(&event.to_clap(self.main_input_port, self.input_prefers_midi));
	}

	pub fn push_all(&mut self, events: impl IntoIterator<Item: EventImpl>) {
		for event in events {
			self.push(event);
		}
	}

	pub fn prepare(&mut self) -> (InputEvents<'_>, OutputEvents<'_>) {
		self.input_events.sort();
		(self.input_events.as_input(), self.output_events.as_output())
	}

	pub fn output_events<Event: EventImpl>(&self) -> impl Iterator<Item = Event> {
		self.output_events
			.iter()
			.filter_map(Event::try_from_unknown)
	}

	pub fn reset(&mut self) {
		self.input_events.clear();
		self.output_events.clear();
	}
}
