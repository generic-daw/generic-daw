use clack_host::{
    events::{
        Event as _, Match, Pckn,
        event_types::{MidiEvent, NoteChokeEvent, NoteOffEvent, NoteOnEvent},
    },
    prelude::*,
};

#[derive(Clone, Copy, Debug)]
pub enum Event {
    On {
        time: u32,
        channel: u8,
        key: u8,
        velocity: f64,
    },
    Off {
        time: u32,
        channel: u8,
        key: u8,
        velocity: f64,
    },
    AllOff {
        time: u32,
        channel: u8,
    },
}

impl Event {
    pub(crate) fn push_as_clap(self, port_index: u16, event_buffer: &mut EventBuffer) {
        match self {
            Self::On {
                time,
                channel,
                key,
                velocity,
            } => event_buffer.push(&NoteOnEvent::new(
                time,
                Pckn::new(port_index, channel, key, Match::All),
                velocity,
            )),
            Self::Off {
                time,
                channel,
                key,
                velocity,
            } => event_buffer.push(&NoteOffEvent::new(
                time,
                Pckn::new(port_index, channel, key, Match::All),
                velocity,
            )),
            Self::AllOff { time, channel } => event_buffer.push(&NoteChokeEvent::new(
                time,
                Pckn::new(port_index, channel, Match::All, Match::All),
            )),
        };
    }

    pub(crate) fn push_as_midi(self, port_index: u16, event_buffer: &mut EventBuffer) {
        match self {
            Self::On {
                time,
                channel,
                key,
                velocity,
            } => {
                event_buffer.push(&MidiEvent::new(
                    time,
                    port_index,
                    [0x90 | channel, key, (velocity * 127.0) as u8],
                ));
            }
            Self::Off {
                time,
                channel,
                key,
                velocity,
            } => {
                event_buffer.push(&MidiEvent::new(
                    time,
                    port_index,
                    [0x80 | channel, key, (velocity * 127.0) as u8],
                ));
            }
            Self::AllOff { time, channel } => {
                event_buffer.push(&MidiEvent::new(
                    time,
                    port_index,
                    [0xb0 | channel, 0x7b, 0x00],
                ));
            }
        }
    }

    fn from_midi(event: &MidiEvent) -> Option<Self> {
        let time = event.time();
        let data = event.data();
        let kind = data[0] & 0xf0;
        let channel = data[0] & 0x0f;
        let key = data[1];
        let velocity = f64::from(data[2]) / 127.0;

        match kind {
            0x90 => Some(Self::On {
                time,
                channel,
                key,
                velocity,
            }),
            0x80 => Some(Self::Off {
                time,
                channel,
                key,
                velocity,
            }),
            0xb0 => Some(Self::AllOff { time, channel }),
            _ => None,
        }
    }

    fn from_clap(value: &UnknownEvent) -> Option<Self> {
        if let Some(event) = value.as_event::<NoteOnEvent>() {
            Some(Self::On {
                time: event.time(),
                channel: *event.channel().as_specific()? as u8,
                key: *event.key().as_specific()? as u8,
                velocity: event.velocity(),
            })
        } else if let Some(event) = value.as_event::<NoteOffEvent>() {
            Some(Self::Off {
                time: event.time(),
                channel: *event.channel().as_specific()? as u8,
                key: *event.key().as_specific()? as u8,
                velocity: event.velocity(),
            })
        } else if let Some(event) = value.as_event::<NoteChokeEvent>() {
            Some(Self::AllOff {
                time: event.time(),
                channel: *event.channel().as_specific()? as u8,
            })
        } else {
            None
        }
    }
}

impl TryFrom<&UnknownEvent> for Event {
    type Error = ();

    fn try_from(value: &UnknownEvent) -> Result<Self, Self::Error> {
        value
            .as_event::<MidiEvent>()
            .map_or_else(|| Self::from_clap(value), Self::from_midi)
            .ok_or(())
    }
}
