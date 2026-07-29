use crate::{
	event_ports_config::EventPortsConfig,
	events::{EventImpl, NoteDialect},
	host::Host,
	param::Param,
};
use clack_extensions::params::ParamInfoFlags;
use clack_host::prelude::*;

#[derive(Debug, Default)]
pub struct EventBuffers {
	input_config: EventPortsConfig,

	input_events: EventBuffer,
	output_events: EventBuffer,
}

impl EventBuffers {
	pub fn new(plugin: &mut PluginInstance<Host>, params: &[Param]) -> Self {
		let input_config = EventPortsConfig::from_ports(plugin, true).unwrap_or_default();

		let event_buffers_cap = params
			.iter()
			.filter(|param| {
				!param
					.flags
					.intersects(ParamInfoFlags::IS_HIDDEN | ParamInfoFlags::IS_READONLY)
			})
			.count() + 128;

		Self {
			input_config,

			input_events: EventBuffer::with_capacity(event_buffers_cap),
			output_events: EventBuffer::with_capacity(event_buffers_cap),
		}
	}

	pub fn are_inputs_empty(&self) -> bool {
		self.input_events.is_empty()
	}

	pub fn are_outputs_empty(&self) -> bool {
		self.output_events.is_empty()
	}

	pub fn push(&mut self, event: impl EventImpl) {
		self.input_events.push(
			&event.to_clap(
				self.input_config
					.preferred_dialects
					.first()
					.copied()
					.unwrap_or(NoteDialect::Clap),
			),
		);
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
