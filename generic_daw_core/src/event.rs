use clap_host::{
	ClapId, Cookie,
	events::{
		self, ClapEvent, Event as _, EventFlags, Match, Midi2Event, MidiEvent, NoteDialect,
		NoteOffEvent, NoteOnEvent, ParamValueEvent, Pckn, UnknownEvent, spaces::CoreEventSpace,
	},
};

#[derive(Clone, Copy, Debug)]
pub enum Event {
	On {
		time: u32,
		key: u8,
		velocity: f32,
		note_id: Match<u32>,
		flags: EventFlags,
	},
	Off {
		time: u32,
		key: u8,
		velocity: f32,
		note_id: Match<u32>,
		flags: EventFlags,
	},
	ParamValue {
		time: u32,
		param_id: ClapId,
		value: f32,
		flags: EventFlags,
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

impl events::EventImpl for Event {
	fn to_clap(self, dialect: NoteDialect) -> ClapEvent {
		match self {
			Self::On {
				time,
				key,
				velocity,
				note_id,
				flags,
			} => match dialect {
				NoteDialect::Clap => ClapEvent::NoteOn(
					NoteOnEvent::new(time, Pckn::new(0u16, 0u16, key, note_id), velocity.into())
						.with_flags(flags),
				),
				NoteDialect::Midi | NoteDialect::MidiMpe => ClapEvent::Midi(
					MidiEvent::new(
						time,
						0u16,
						[0x90, key, (velocity * 127.0).round().max(1.0) as u8],
					)
					.with_flags(flags),
				),
				NoteDialect::Midi2 => ClapEvent::Midi2(
					Midi2Event::new(
						time,
						0u16,
						[
							(0x4090 << 16) | (u32::from(key) << 8),
							(u32::from((velocity * 65535.0).round() as u16) << 16),
							0,
							0,
						],
					)
					.with_flags(flags),
				),
			},
			Self::Off {
				time,
				key,
				velocity,
				note_id,
				flags,
			} => match dialect {
				NoteDialect::Clap => ClapEvent::NoteOff(
					NoteOffEvent::new(time, Pckn::new(0u16, 0u16, key, note_id), velocity.into())
						.with_flags(flags),
				),
				NoteDialect::Midi | NoteDialect::MidiMpe => ClapEvent::Midi(
					MidiEvent::new(time, 0u16, [0x80, key, (velocity * 127.0).round() as u8])
						.with_flags(flags),
				),
				NoteDialect::Midi2 => ClapEvent::Midi2(
					Midi2Event::new(
						time,
						0u16,
						[
							(0x4080 << 16) | (u32::from(key) << 8),
							(u32::from((velocity * 65535.0).round() as u16) << 16),
							0,
							0,
						],
					)
					.with_flags(flags),
				),
			},
			Self::ParamValue {
				time,
				param_id,
				value,
				flags,
			} => ClapEvent::ParamValue(
				ParamValueEvent::new(
					time,
					param_id,
					Pckn::new(0u16, 0u16, Match::All, Match::All),
					value.into(),
					Cookie::empty(),
				)
				.with_flags(flags),
			),
		}
	}

	fn try_from_unknown(value: &UnknownEvent) -> Option<Self> {
		match value.as_core_event()? {
			CoreEventSpace::NoteOn(event) => Some(Self::On {
				time: event.time(),
				key: *event.key().as_specific()? as u8,
				velocity: event.velocity() as f32,
				note_id: event.note_id(),
				flags: event.flags(),
			}),
			CoreEventSpace::NoteOff(event) => Some(Self::Off {
				time: event.time(),
				key: *event.key().as_specific()? as u8,
				velocity: event.velocity() as f32,
				note_id: event.note_id(),
				flags: event.flags(),
			}),
			CoreEventSpace::ParamValue(event) => Some(Self::ParamValue {
				time: event.time(),
				param_id: event.param_id()?,
				value: event.value() as f32,
				flags: event.flags(),
			}),
			CoreEventSpace::Midi(event) => {
				let data = event.data();
				match data[0] & 0xf0 {
					0x90 if data[2] != 0 => Some(Self::On {
						time: event.time(),
						key: data[1],
						velocity: f32::from(data[2]) / 127.0,
						note_id: Match::All,
						flags: event.flags(),
					}),
					0x90 | 0x80 => Some(Self::Off {
						time: event.time(),
						key: data[1],
						velocity: f32::from(data[2]) / 127.0,
						note_id: Match::All,
						flags: event.flags(),
					}),
					_ => None,
				}
			}
			CoreEventSpace::Midi2(event) => {
				let data = event.data();
				match data[0] >> 16 {
					0x4090 => Some(Self::On {
						time: event.time(),
						key: (data[0] >> 8) as u8,
						velocity: f32::from((data[1] >> 16) as u16) / 65535.0,
						note_id: Match::All,
						flags: event.flags(),
					}),
					0x4080 => Some(Self::Off {
						time: event.time(),
						key: (data[0] >> 8) as u8,
						velocity: f32::from((data[1] >> 16) as u16) / 65535.0,
						note_id: Match::All,
						flags: event.flags(),
					}),
					_ => None,
				}
			}
			_ => None,
		}
	}
}
