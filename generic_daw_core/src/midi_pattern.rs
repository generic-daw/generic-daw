use crate::{
	MidiKey, MidiNote, Transport,
	midi_note::MidiNoteId,
	time::{BeatRange, BeatTime, SecondsTime},
};
use midly::{
	Format, MidiMessage, Timing, TrackEventKind,
	num::{u7, u28},
};
use std::collections::HashMap;
use utils::unique_id;

unique_id!(midi_pattern_id);

pub use midi_pattern_id::Id as MidiPatternId;

#[derive(Clone, Copy, Debug)]
pub enum MidiPatternAction {
	Add(MidiNote),
	Remove(MidiNoteId),
	ChangeKey(MidiNoteId, MidiKey),
	ChangeVelocity(MidiNoteId, f32),
	MoveTo(MidiNoteId, BeatTime),
	TrimStartTo(MidiNoteId, BeatTime),
	TrimEndTo(MidiNoteId, BeatTime),
}

#[derive(Clone, Debug)]
pub struct MidiPattern {
	pub id: MidiPatternId,
	pub notes: HashMap<MidiNoteId, MidiNote>,
}

impl MidiPattern {
	#[must_use]
	pub fn from_notes(notes: &[MidiNote]) -> Self {
		Self {
			id: MidiPatternId::unique(),
			notes: notes.iter().copied().map(|note| (note.id, note)).collect(),
		}
	}

	#[must_use]
	pub fn parse_midi(bytes: &[u8], transport: &Transport) -> Option<Vec<MidiNote>> {
		#[derive(Clone, Copy, Default, Eq, PartialEq)]
		enum Entry {
			Some(u28, u7),
			#[default]
			None,
		}

		let (header, tracks) = midly::parse(bytes).ok()?;

		let midi_tick_to_musical_time = |tick: u32| match header.timing {
			Timing::Metrical(ticks_per_beat) => {
				let ticks_per_beat = u32::from(ticks_per_beat.as_int());
				BeatTime::new(
					u64::from(tick / ticks_per_beat),
					((tick % ticks_per_beat) as f32 / ticks_per_beat as f32
						* BeatTime::FACTOR as f32) as u64,
				)
			}
			Timing::Timecode(fps, subframe) => SecondsTime::from_float(
				f64::from(tick) / f64::from(fps.as_f32()) / f64::from(subframe),
			)
			.to_beat_time(transport),
		};

		let mut notes = Vec::new();
		let mut playing = [[Entry::None; 128]; 4];

		let mut time = u28::new(0);
		for track in tracks {
			let track = track.ok()?;
			for event in track {
				let event = event.ok()?;

				time += event.delta;

				match event.kind {
					TrackEventKind::Midi {
						message: MidiMessage::NoteOn { key, vel },
						channel,
					} => {
						let entry =
							&mut playing[usize::from(channel.as_int())][usize::from(key.as_int())];

						if *entry == Entry::None {
							*entry = Entry::Some(time, vel);
						}
					}
					TrackEventKind::Midi {
						message: MidiMessage::NoteOff { key, .. },
						channel,
					} => {
						let entry =
							&mut playing[usize::from(channel.as_int())][usize::from(key.as_int())];

						if let Entry::Some(start, vel) = std::mem::take(entry) {
							let note = MidiNote {
								key: MidiKey(key.as_int()),
								velocity: f32::from(vel.as_int()) / 127.0,
								position: BeatRange::new(
									midi_tick_to_musical_time(start.as_int()),
									midi_tick_to_musical_time(time.as_int()),
								),
								id: MidiNoteId::unique(),
							};

							notes.push(note);
						}
					}
					_ => {}
				}
			}

			for (key, entry) in playing
				.iter_mut()
				.flat_map(|playing| playing.iter_mut().enumerate())
			{
				if let Entry::Some(start, vel) = std::mem::take(entry) {
					notes.push(MidiNote {
						key: MidiKey(key as u8),
						velocity: f32::from(vel.as_int()) / 127.0,
						position: BeatRange::new(
							midi_tick_to_musical_time(start.as_int()),
							midi_tick_to_musical_time(time.as_int()),
						),
						id: MidiNoteId::unique(),
					});
				}
			}

			if header.format == Format::Parallel {
				time = u28::new(0);
			}
		}

		Some(notes)
	}

	pub fn apply(&mut self, action: MidiPatternAction) {
		match action {
			MidiPatternAction::Add(note) => _ = self.notes.insert(note.id, note),
			MidiPatternAction::Remove(id) => _ = self.notes.remove(&id),
			MidiPatternAction::ChangeKey(id, key) => self.notes.get_mut(&id).unwrap().key = key,
			MidiPatternAction::ChangeVelocity(id, velocity) => {
				self.notes.get_mut(&id).unwrap().velocity = velocity;
			}
			MidiPatternAction::MoveTo(id, pos) => {
				self.notes.get_mut(&id).unwrap().position.move_to(pos);
			}
			MidiPatternAction::TrimStartTo(id, pos) => {
				self.notes.get_mut(&id).unwrap().position.trim_start_to(pos);
			}
			MidiPatternAction::TrimEndTo(id, pos) => {
				self.notes.get_mut(&id).unwrap().position.trim_end_to(pos);
			}
		}
	}
}
