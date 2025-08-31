use clap_host::{
	ClapId, Cookie,
	events::{
		ClapEvent, Event as _, Match, MidiEvent, NoteChokeEvent, NoteOffEvent, NoteOnEvent,
		ParamValueEvent, Pckn, UnknownEvent,
	},
};

#[derive(Clone, Copy, Debug)]
pub enum Event {
	On {
		time: u32,
		channel: u8,
		key: u8,
		velocity: f32,
	},
	Off {
		time: u32,
		channel: u8,
		key: u8,
		velocity: f32,
	},
	AllOff {
		time: u32,
		channel: u8,
	},
	ParamValue {
		time: u32,
		param_id: ClapId,
		channel: u8,
		value: f32,
		cookie: Cookie,
	},
}

impl audio_graph::EventImpl for Event {
	fn time(self) -> usize {
		match self {
			Self::On { time, .. }
			| Self::Off { time, .. }
			| Self::AllOff { time, .. }
			| Self::ParamValue { time, .. } => time as usize * 2,
		}
	}

	fn with_time(mut self, to: usize) -> Self {
		match &mut self {
			Self::On { time, .. }
			| Self::Off { time, .. }
			| Self::AllOff { time, .. }
			| Self::ParamValue { time, .. } => {
				*time = (to / 2) as u32;
			}
		}
		self
	}
}

impl clap_host::EventImpl for Event {
	fn to_clap(self, port_index: u16) -> ClapEvent {
		match self {
			Self::On {
				time,
				channel,
				key,
				velocity,
			} => ClapEvent::NoteOnEvent(NoteOnEvent::new(
				time,
				Pckn::new(port_index, channel, key, Match::All),
				velocity.into(),
			)),
			Self::Off {
				time,
				channel,
				key,
				velocity,
			} => ClapEvent::NoteOffEvent(NoteOffEvent::new(
				time,
				Pckn::new(port_index, channel, key, Match::All),
				velocity.into(),
			)),
			Self::AllOff { time, channel } => ClapEvent::NoteChokeEvent(NoteChokeEvent::new(
				time,
				Pckn::new(port_index, channel, Match::All, Match::All),
			)),
			Self::ParamValue {
				time,
				param_id,
				channel,
				value,
				cookie,
			} => ClapEvent::ParamValueEvent(ParamValueEvent::new(
				time,
				param_id,
				Pckn::new(port_index, channel, Match::All, Match::All),
				value.into(),
				cookie,
			)),
		}
	}

	fn to_midi(self, port_index: u16) -> MidiEvent {
		match self {
			Self::On {
				time,
				channel,
				key,
				velocity,
			} => MidiEvent::new(
				time,
				port_index,
				[0x90 | channel, key, (velocity * 127.0) as u8],
			),
			Self::Off {
				time,
				channel,
				key,
				velocity,
			} => MidiEvent::new(
				time,
				port_index,
				[0x80 | channel, key, (velocity * 127.0) as u8],
			),
			Self::AllOff { time, channel } => {
				MidiEvent::new(time, port_index, [0xb0 | channel, 0x7b, 0x00])
			}
			Self::ParamValue {
				time,
				param_id,
				channel,
				value,
				cookie: _,
			} => MidiEvent::new(
				time,
				port_index,
				[0xb0 | channel, param_id.get() as u8, (value * 127.0) as u8],
			),
		}
	}

	fn try_from_unknown(value: &UnknownEvent) -> Option<Self> {
		value
			.as_event::<MidiEvent>()
			.map_or_else(|| Self::from_clap(value), Self::from_midi)
	}
}

impl Event {
	fn from_midi(event: &MidiEvent) -> Option<Self> {
		let time = event.time();
		let data = event.data();
		let kind = data[0] & 0xf0;
		let channel = data[0] & 0x0f;
		let bytes = data[1];
		let value = f32::from(data[2]) / 127.0;

		match kind {
			0x90 => Some(Self::On {
				time,
				channel,
				key: bytes,
				velocity: value,
			}),
			0x80 => Some(Self::Off {
				time,
				channel,
				key: bytes,
				velocity: value,
			}),
			0xb0 if bytes == 0x7b => Some(Self::AllOff { time, channel }),
			0xb0 if bytes < 0x78 => Some(Self::ParamValue {
				time,
				param_id: ClapId::from_raw(bytes.into())?,
				channel,
				value,
				cookie: Cookie::empty(),
			}),
			_ => None,
		}
	}

	fn from_clap(value: &UnknownEvent) -> Option<Self> {
		if let Some(event) = value.as_event::<NoteOnEvent>() {
			Some(Self::On {
				time: event.time(),
				channel: *event.channel().as_specific()? as u8,
				key: *event.key().as_specific()? as u8,
				velocity: event.velocity() as f32,
			})
		} else if let Some(event) = value.as_event::<NoteOffEvent>() {
			Some(Self::Off {
				time: event.time(),
				channel: *event.channel().as_specific()? as u8,
				key: *event.key().as_specific()? as u8,
				velocity: event.velocity() as f32,
			})
		} else if let Some(event) = value.as_event::<NoteChokeEvent>() {
			Some(Self::AllOff {
				time: event.time(),
				channel: *event.channel().as_specific()? as u8,
			})
		} else if let Some(event) = value.as_event::<ParamValueEvent>() {
			Some(Self::ParamValue {
				time: event.time(),
				param_id: event.param_id()?,
				channel: *event.channel().as_specific()? as u8,
				value: event.value() as f32,
				cookie: event.cookie(),
			})
		} else {
			None
		}
	}
}
