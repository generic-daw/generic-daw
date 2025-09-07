use crate::{EventImpl, MainThreadMessage, events::ClapEvent, host::Host};
use async_channel::Sender;
use clack_extensions::note_ports::{NoteDialect, NotePortInfoBuffer};
use clack_host::{
	events::{
		Match,
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
		let ports = *plugin.access_shared_handler(|s| s.note_ports.get())?;

		let mut buffer = NotePortInfoBuffer::new();

		(0..ports
			.count(&mut plugin.plugin_handle(), is_input)
			.min(u16::MAX.into()))
			.find_map(|i| {
				let port = ports.get(&mut plugin.plugin_handle(), i, is_input, &mut buffer)?;

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

	pub fn write_out<Event: EventImpl>(
		&mut self,
		events: &mut Vec<Event>,
		sender: &Sender<MainThreadMessage>,
	) {
		for unknown in &self.output_events {
			let Some(event) = Event::try_from_unknown(unknown) else {
				continue;
			};

			// the reference host handles:
			// - CLAP_EVENT_PARAM_VALUE
			// - CLAP_EVENT_PARAM_GESTURE_BEGIN (we don't have this)
			// - CLAP_EVENT_PARAM_GESTURE_END (we don't have this)
			if let ClapEvent::ParamValue(event) = event.to_clap(0)
				&& let Some(id) = event.param_id()
			{
				sender
					.try_send(MainThreadMessage::ParamChanged(id, event.value() as f32))
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
