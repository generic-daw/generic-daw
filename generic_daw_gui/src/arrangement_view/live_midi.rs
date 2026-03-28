use generic_daw_core::{Event, NoteId};
use iced::keyboard::key::{Code, Physical};
use std::collections::{HashMap, VecDeque};

#[derive(Debug)]
pub struct LiveMidiState {
	external_active: HashMap<(u8, u8), VecDeque<NoteId>>,
	typing_active: HashMap<Physical, (u8, NoteId)>,
	typing_octave: i8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypingKey {
	Note(u8),
	OctaveDown,
	OctaveUp,
}

impl Default for LiveMidiState {
	fn default() -> Self {
		Self {
			external_active: HashMap::new(),
			typing_active: HashMap::new(),
			typing_octave: 4,
		}
	}
}

impl LiveMidiState {
	pub fn note_on(&mut self, channel: u8, key: u8, velocity: f32) -> Event {
		let note_id = NoteId::unique();
		self.external_active
			.entry((channel, key))
			.or_default()
			.push_back(note_id);
		Event::On {
			time: 0,
			key,
			velocity,
			note_id,
		}
	}

	pub fn note_off(&mut self, channel: u8, key: u8, velocity: f32) -> Option<Event> {
		let active = self.external_active.get_mut(&(channel, key))?;
		let note_id = active.pop_front()?;
		if active.is_empty() {
			self.external_active.remove(&(channel, key));
		}
		Some(Event::Off {
			time: 0,
			key,
			velocity,
			note_id,
		})
	}

	pub fn typing_press(&mut self, physical: Physical) -> Option<Event> {
		match map_typing_key(physical, self.typing_octave)? {
			TypingKey::Note(key) => {
				if self.typing_active.contains_key(&physical) {
					return None;
				}

				let note_id = NoteId::unique();
				self.typing_active.insert(physical, (key, note_id));
				Some(Event::On {
					time: 0,
					key,
					velocity: 1.0,
					note_id,
				})
			}
			TypingKey::OctaveDown => {
				self.typing_octave = (self.typing_octave - 1).clamp(-1, 8);
				None
			}
			TypingKey::OctaveUp => {
				self.typing_octave = (self.typing_octave + 1).clamp(-1, 8);
				None
			}
		}
	}

	pub fn typing_release(&mut self, physical: Physical) -> Option<Event> {
		let (key, note_id) = self.typing_active.remove(&physical)?;
		Some(Event::Off {
			time: 0,
			key,
			velocity: 1.0,
			note_id,
		})
	}

	pub fn release_all(&mut self) -> Vec<Event> {
		let mut events = self
			.typing_active
			.drain()
			.map(|(_, (key, note_id))| Event::Off {
				time: 0,
				key,
				velocity: 1.0,
				note_id,
			})
			.collect::<Vec<_>>();

		events.extend(
			self.external_active
				.drain()
				.flat_map(|((_, key), note_ids)| {
					note_ids.into_iter().map(move |note_id| Event::Off {
						time: 0,
						key,
						velocity: 0.0,
						note_id,
					})
				}),
		);

		events
	}
}

fn map_typing_key(physical: Physical, octave: i8) -> Option<TypingKey> {
	let Physical::Code(code) = physical else {
		return None;
	};

	let semitone = match code {
		Code::KeyA => Some(0),
		Code::KeyW => Some(1),
		Code::KeyS => Some(2),
		Code::KeyE => Some(3),
		Code::KeyD => Some(4),
		Code::KeyF => Some(5),
		Code::KeyT => Some(6),
		Code::KeyG => Some(7),
		Code::KeyY => Some(8),
		Code::KeyH => Some(9),
		Code::KeyU => Some(10),
		Code::KeyJ => Some(11),
		Code::KeyK => Some(12),
		Code::KeyZ => return Some(TypingKey::OctaveDown),
		Code::KeyX => return Some(TypingKey::OctaveUp),
		_ => None,
	}?;

	let key = i16::from(octave + 1) * 12 + semitone;
	(0..=127)
		.contains(&key)
		.then_some(TypingKey::Note(key as u8))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn external_note_off_matches_oldest_active_voice() {
		let mut state = LiveMidiState::default();
		let first = state.note_on(0, 60, 1.0);
		let second = state.note_on(0, 60, 0.5);

		let Some(Event::Off { note_id, .. }) = state.note_off(0, 60, 1.0) else {
			panic!("missing note off");
		};
		let Event::On {
			note_id: first_id, ..
		} = first
		else {
			unreachable!();
		};
		let Event::On {
			note_id: second_id, ..
		} = second
		else {
			unreachable!();
		};

		assert_eq!(note_id, first_id);

		let Some(Event::Off { note_id, .. }) = state.note_off(0, 60, 1.0) else {
			panic!("missing second note off");
		};
		assert_eq!(note_id, second_id);
	}

	#[test]
	fn typing_keyboard_tracks_press_and_release() {
		let mut state = LiveMidiState::default();

		let Some(Event::On { key, note_id, .. }) = state.typing_press(Physical::Code(Code::KeyA))
		else {
			panic!("missing note on");
		};
		assert_eq!(key, 60);

		assert!(state.typing_press(Physical::Code(Code::KeyA)).is_none());

		let Some(Event::Off {
			key: off_key,
			note_id: off_note_id,
			..
		}) = state.typing_release(Physical::Code(Code::KeyA))
		else {
			panic!("missing note off");
		};
		assert_eq!(off_key, key);
		assert_eq!(off_note_id, note_id);
	}
}
