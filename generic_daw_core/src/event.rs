use crate::clap_host::{
	ClapId, Cookie,
	events::{
		ClapEvent, Event as _, Match, MidiEvent, NoteChokeEvent, NoteEndEvent, NoteOffEvent,
		NoteOnEvent, ParamValueEvent, Pckn, UnknownEvent,
	},
};

#[derive(Clone, Copy, Debug)]
pub enum Event {
	On {
		time: u32,
		key: u8,
		velocity: f32,
	},
	Off {
		time: u32,
		key: u8,
		velocity: f32,
	},
	Choke {
		time: u32,
		key: u8,
	},
	End {
		time: u32,
		key: u8,
	},
	ParamValue {
		time: u32,
		param_id: ClapId,
		value: f32,
		cookie: Cookie,
	},
}

impl audio_graph::EventImpl for Event {
	fn time(self) -> usize {
		match self {
			Self::On { time, .. }
			| Self::Off { time, .. }
			| Self::Choke { time, .. }
			| Self::End { time, .. }
			| Self::ParamValue { time, .. } => time as usize,
		}
	}

	fn with_time(mut self, to: usize) -> Self {
		match &mut self {
			Self::On { time, .. }
			| Self::Off { time, .. }
			| Self::Choke { time, .. }
			| Self::End { time, .. }
			| Self::ParamValue { time, .. } => {
				*time = to as u32;
			}
		}
		self
	}
}

impl clap_host::EventImpl for Event {
	fn to_clap(&self, port_index: u16) -> ClapEvent {
		match *self {
			Self::On {
				time,
				key,
				velocity,
			} => ClapEvent::NoteOn(NoteOnEvent::new(
				time,
				Pckn::new(port_index, Match::All, key, Match::All),
				velocity.into(),
			)),
			Self::Off {
				time,
				key,
				velocity,
			} => ClapEvent::NoteOff(NoteOffEvent::new(
				time,
				Pckn::new(port_index, Match::All, key, Match::All),
				velocity.into(),
			)),
			Self::Choke { time, key } => ClapEvent::NoteChoke(NoteChokeEvent::new(
				time,
				Pckn::new(port_index, Match::All, key, Match::All),
			)),
			Self::End { time, key } => ClapEvent::NoteEnd(NoteEndEvent::new(
				time,
				Pckn::new(port_index, Match::All, key, Match::All),
			)),
			Self::ParamValue {
				time,
				param_id,
				value,
				cookie,
			} => ClapEvent::ParamValue(ParamValueEvent::new(
				time,
				param_id,
				Pckn::new(port_index, Match::All, Match::All, Match::All),
				value.into(),
				cookie,
			)),
		}
	}

	fn to_midi(&self, port_index: u16) -> MidiEvent {
		match *self {
			Self::On {
				time,
				key,
				velocity,
			} => MidiEvent::new(time, port_index, [0x90, key, (velocity * 127.0) as u8]),
			Self::Off {
				time,
				key,
				velocity,
			} => MidiEvent::new(time, port_index, [0x80, key, (velocity * 127.0) as u8]),
			Self::Choke { time, key } | Self::End { time, key } => Self::Off {
				time,
				key,
				velocity: 1.0,
			}
			.to_midi(port_index),
			Self::ParamValue {
				time,
				param_id,
				value,
				..
			} => MidiEvent::new(
				time,
				port_index,
				[0xb0, param_id.get() as u8, (value * 127.0) as u8],
			),
		}
	}

	fn try_from_unknown(value: &UnknownEvent) -> Option<Self> {
		if let Some(event) = value.as_event::<MidiEvent>() {
			let time = event.time();
			let data = event.data();
			let value = f32::from(data[2]) / 127.0;

			match data[0] & 0xf0 {
				0x90 => Some(Self::On {
					time,
					key: data[1],
					velocity: value,
				}),
				0x80 => Some(Self::Off {
					time,
					key: data[1],
					velocity: value,
				}),
				0xb0 if data[1] < 0x78 => Some(Self::ParamValue {
					time,
					param_id: ClapId::from_raw(data[1].into())?,
					value,
					cookie: Cookie::empty(),
				}),
				_ => None,
			}
		} else if let Some(event) = value.as_event::<NoteOnEvent>() {
			Some(Self::On {
				time: event.time(),
				key: *event.key().as_specific()? as u8,
				velocity: event.velocity() as f32,
			})
		} else if let Some(event) = value.as_event::<NoteOffEvent>() {
			Some(Self::Off {
				time: event.time(),
				key: *event.key().as_specific()? as u8,
				velocity: event.velocity() as f32,
			})
		} else if let Some(event) = value.as_event::<NoteChokeEvent>() {
			Some(Self::Choke {
				time: event.time(),
				key: *event.key().as_specific()? as u8,
			})
		} else if let Some(event) = value.as_event::<NoteEndEvent>() {
			Some(Self::End {
				time: event.time(),
				key: *event.key().as_specific()? as u8,
			})
		} else if let Some(event) = value.as_event::<ParamValueEvent>() {
			Some(Self::ParamValue {
				time: event.time(),
				param_id: event.param_id()?,
				value: event.value() as f32,
				cookie: event.cookie(),
			})
		} else {
			None
		}
	}
}
