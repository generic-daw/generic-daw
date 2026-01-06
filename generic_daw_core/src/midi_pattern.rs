use crate::{MidiKey, MidiNote, MusicalTime, Position, Transport};
use log::warn;
use midly::{
	Format, MidiMessage, Timing, TrackEventKind,
	num::{u7, u28},
};
use std::time::Duration;
use utils::unique_id;

unique_id!(midi_pattern_id);

pub use midi_pattern_id::Id as MidiPatternId;

#[derive(Clone, Copy, Debug)]
pub enum MidiPatternAction {
	Add(MidiNote, usize),
	Remove(usize),
	ChangeKey(usize, MidiKey),
	ChangeVelocity(usize, f32),
	MoveTo(usize, MusicalTime),
	TrimStartTo(usize, MusicalTime),
	TrimEndTo(usize, MusicalTime),
}

#[derive(Debug)]
pub struct MidiPattern {
	pub id: MidiPatternId,
	pub notes: Vec<MidiNote>,
}

impl MidiPattern {
	#[must_use]
	pub fn from_notes(notes: Vec<MidiNote>) -> Self {
		Self {
			id: MidiPatternId::unique(),
			notes,
		}
	}

	#[must_use]
	pub fn from_midi(bytes: &[u8], transport: &Transport) -> Option<Self> {
		#[derive(Clone, Copy, Default)]
		enum Entry {
			Some(u28, u7),
			#[default]
			None,
		}

		let (header, tracks) = midly::parse(bytes).ok()?;

		let midi_tick_to_musical_time = |tick: u28| match header.timing {
			Timing::Metrical(ticks_per_beat) => {
				let tick = tick.as_int();
				let ticks_per_beat = u32::from(ticks_per_beat.as_int());
				MusicalTime::new(
					u64::from(tick / ticks_per_beat),
					((tick % ticks_per_beat) as f32 / ticks_per_beat as f32
						* MusicalTime::TICKS_PER_BEAT as f32) as u64,
				)
			}
			Timing::Timecode(fps, subframe) => MusicalTime::from_duration(
				Duration::from_secs_f32(1.0 / fps.as_f32() / f32::from(subframe)),
				transport,
			),
		};

		let mut notes = Vec::new();
		let mut playing = [Entry::None; 128];

		let mut time = u28::new(0);
		for track in tracks {
			let track = track.ok()?;
			for event in track {
				let event = event.ok()?;

				time += event.delta;

				match event.kind {
					TrackEventKind::Midi {
						message: MidiMessage::NoteOn { key, vel },
						..
					} => {
						let entry = &mut playing[usize::from(key.as_int())];

						if matches!(entry, Entry::None) {
							*entry = Entry::Some(time, vel);
						}
					}
					TrackEventKind::Midi {
						message: MidiMessage::NoteOff { key, .. },
						..
					} => {
						let entry = &mut playing[usize::from(key.as_int())];

						if let Entry::Some(start, vel) = std::mem::take(entry) {
							let note = MidiNote {
								key: MidiKey(key.as_int()),
								velocity: f32::from(vel.as_int()) / 127.0,
								position: Position::new(
									midi_tick_to_musical_time(start),
									midi_tick_to_musical_time(time),
								),
							};

							notes.push(note);
						}
					}
					_ => {}
				}
			}

			for (key, entry) in playing.iter_mut().enumerate() {
				if let Entry::Some(start, vel) = std::mem::take(entry) {
					let note = MidiNote {
						key: MidiKey(key as u8),
						velocity: f32::from(vel.as_int()) / 127.0,
						position: Position::new(
							midi_tick_to_musical_time(start),
							midi_tick_to_musical_time(time),
						),
					};

					warn!("note {note:?} wasn't ended");

					notes.push(note);
				}
			}

			if matches!(header.format, Format::Parallel) {
				time = u28::new(0);
			}
		}

		Some(Self {
			id: MidiPatternId::unique(),
			notes,
		})
	}

	pub fn apply(&mut self, action: MidiPatternAction) {
		match action {
			MidiPatternAction::Add(note, index) => self.notes.insert(index, note),
			MidiPatternAction::Remove(index) => _ = self.notes.remove(index),
			MidiPatternAction::ChangeKey(index, key) => self.notes[index].key = key,
			MidiPatternAction::ChangeVelocity(index, velocity) => {
				self.notes[index].velocity = velocity;
			}
			MidiPatternAction::MoveTo(index, pos) => self.notes[index].position.move_to(pos),
			MidiPatternAction::TrimStartTo(index, pos) => {
				self.notes[index].position.trim_start_to(pos);
			}
			MidiPatternAction::TrimEndTo(index, pos) => self.notes[index].position.trim_end_to(pos),
		}
	}
}
