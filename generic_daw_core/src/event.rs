use crate::clap_host::{
	ClapId, Cookie,
	events::{
		ClapEvent, Event as _, Match, MidiEvent, NoteOffEvent, NoteOnEvent, ParamValueEvent, Pckn,
		UnknownEvent, spaces::CoreEventSpace,
	},
};

#[derive(Clone, Copy, Debug)]
pub enum Event {
	On {
		time: u32,
		key: u8,
		velocity: f32,
		note_id: Match<u32>,
	},
	Off {
		time: u32,
		key: u8,
		velocity: f32,
		note_id: Match<u32>,
	},
	ParamValue {
		time: u32,
		param_id: ClapId,
		value: f32,
		cookie: Cookie,
	},
}

impl audio_graph::EventImpl for Event {
	fn time(&self) -> usize {
		match *self {
			Self::On { time, .. } | Self::Off { time, .. } | Self::ParamValue { time, .. } => {
				time as usize
			}
		}
	}

	fn at(&self, at: usize) -> Self {
		let mut this = *self;
		match &mut this {
			Self::On { time, .. } | Self::Off { time, .. } | Self::ParamValue { time, .. } => {
				*time = at as u32;
			}
		}
		this
	}
}

impl clap_host::EventImpl for Event {
	fn to_clap(self, port_index: u16, prefers_midi: bool) -> ClapEvent {
		match self {
			Self::On {
				time,
				key,
				velocity,
				note_id: _,
			} if prefers_midi => ClapEvent::Midi(MidiEvent::new(
				time,
				port_index,
				[0x90, key, (velocity * 127.0).round() as u8],
			)),
			Self::Off {
				time,
				key,
				velocity,
				note_id: _,
			} if prefers_midi => ClapEvent::Midi(MidiEvent::new(
				time,
				port_index,
				[0x80, key, (velocity * 127.0).round() as u8],
			)),
			Self::On {
				time,
				key,
				velocity,
				note_id,
			} => ClapEvent::NoteOn(NoteOnEvent::new(
				time,
				Pckn::new(port_index, 0u16, key, note_id),
				velocity.into(),
			)),
			Self::Off {
				time,
				key,
				velocity,
				note_id,
			} => ClapEvent::NoteOff(NoteOffEvent::new(
				time,
				Pckn::new(port_index, 0u16, key, note_id),
				velocity.into(),
			)),
			Self::ParamValue {
				time,
				param_id,
				value,
				cookie,
			} => ClapEvent::ParamValue(ParamValueEvent::new(
				time,
				param_id,
				Pckn::new(port_index, 0u16, Match::All, Match::All),
				value.into(),
				cookie,
			)),
		}
	}

	fn try_from_unknown(value: &UnknownEvent) -> Option<Self> {
		match value.as_core_event()? {
			CoreEventSpace::NoteOn(event) => Some(Self::On {
				time: event.time(),
				key: *event.key().as_specific()? as u8,
				velocity: event.velocity() as f32,
				note_id: event.note_id(),
			}),
			CoreEventSpace::NoteOff(event) => Some(Self::Off {
				time: event.time(),
				key: *event.key().as_specific()? as u8,
				velocity: event.velocity() as f32,
				note_id: event.note_id(),
			}),
			CoreEventSpace::ParamValue(event) => Some(Self::ParamValue {
				time: event.time(),
				param_id: event.param_id()?,
				value: event.value() as f32,
				cookie: event.cookie(),
			}),
			CoreEventSpace::Midi(event) => {
				let data = event.data();
				match data[0] & 0xf0 {
					0x90 => Some(Self::On {
						time: event.time(),
						key: data[1],
						velocity: f32::from(data[2]) / 127.0,
						note_id: Match::All,
					}),
					0x80 => Some(Self::Off {
						time: event.time(),
						key: data[1],
						velocity: f32::from(data[2]) / 127.0,
						note_id: Match::All,
					}),
					_ => None,
				}
			}
			_ => None,
		}
	}
}
