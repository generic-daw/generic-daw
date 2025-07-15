use crate::{ClipPosition, RtState, event::Event};
use arc_swap::ArcSwap;
use generic_daw_utils::NoDebug;
use std::{
	cmp::Ordering,
	iter::repeat_n,
	sync::{Arc, Mutex},
};

mod midi_key;
mod midi_note;

pub use midi_key::{Key, MidiKey};
pub use midi_note::MidiNote;

#[derive(Clone, Debug)]
pub struct MidiClip {
	pub pattern: Arc<ArcSwap<Vec<MidiNote>>>,
	pub position: ClipPosition,
	notes: NoDebug<Arc<Mutex<[[u8; 16]; 128]>>>,
}

impl MidiClip {
	#[must_use]
	pub fn create(pattern: Arc<ArcSwap<Vec<MidiNote>>>) -> Arc<Self> {
		let len = pattern
			.load()
			.iter()
			.map(|note| note.end)
			.max()
			.unwrap_or_default();

		Arc::new(Self {
			pattern,
			position: ClipPosition::with_len(len),
			notes: Arc::new(Mutex::new([[0; 16]; 128])).into(),
		})
	}

	pub fn process(&self, rtstate: &RtState, audio: &[f32], events: &mut Vec<Event>) {
		let start = self.position.start();
		let end = self.position.end();
		let offset = self.position.offset();

		let start_sample = rtstate.sample;
		let end_sample = start_sample + audio.len();

		let mut notes = [[0u8; 16]; 128];

		if rtstate.playing {
			self.pattern
				.load()
				.iter()
				.filter_map(|&note| {
					(note + start)
						.saturating_sub(offset)
						.and_then(|note| note.clamp(start, end))
				})
				.for_each(|note| {
					let start = note.start.to_samples(rtstate);
					let end = note.end.to_samples(rtstate);

					if start < start_sample && end >= start_sample {
						notes[note.key.0 as usize][note.channel as usize] += 1;
					}
				});
		}

		let mut lock = self
			.notes
			.try_lock()
			.expect("this is only locked from the audio thread");

		lock.iter()
			.zip(notes)
			.enumerate()
			.flat_map(|(a, (b, c))| b.iter().zip(c).enumerate().map(move |b| (a, b)))
			.for_each(|(key, (channel, (before, after)))| {
				let event = match before.cmp(&after) {
					Ordering::Equal => return,
					Ordering::Less => Event::On {
						time: 0,
						channel: channel as u8,
						key: key as u8,
						velocity: 1.0,
					},
					Ordering::Greater => Event::Off {
						time: 0,
						channel: channel as u8,
						key: key as u8,
						velocity: 1.0,
					},
				};

				events.extend(repeat_n(event, before.abs_diff(after) as usize));
			});

		if rtstate.playing {
			self.pattern
				.load()
				.iter()
				.filter_map(|&note| {
					(note + start)
						.saturating_sub(offset)
						.and_then(|note| note.clamp(start, end))
				})
				.for_each(|note| {
					let start = note.start.to_samples(rtstate);
					let end = note.end.to_samples(rtstate);

					if start >= start_sample && start < end_sample {
						events.push(Event::On {
							time: (start - start_sample) as u32 / 2,
							channel: note.channel,
							key: note.key.0,
							velocity: note.velocity,
						});
						notes[note.key.0 as usize][note.channel as usize] += 1;
					}

					if end >= start_sample && end < end_sample {
						events.push(Event::Off {
							time: (end - start_sample) as u32 / 2,
							channel: note.channel,
							key: note.key.0,
							velocity: note.velocity,
						});
						notes[note.key.0 as usize][note.channel as usize] -= 1;
					}
				});
		}

		*lock = notes;
	}
}
