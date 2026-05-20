use crate::{
	Event, MidiKey, MidiNote, MidiNoteId, MidiPatternId, audio_thread::State,
	time::OffsetBeatRange, voice_alloc::VoiceAlloc,
};
use clap_host::events::Match;
use utils::unique_id;

unique_id!(midi_clip_id);

pub use midi_clip_id::Id as MidiClipId;
pub type VoiceId = (MidiClipId, MidiNoteId, MidiKey);

#[derive(Clone, Copy, Debug)]
pub struct MidiClip {
	pub id: MidiClipId,
	pub pattern: MidiPatternId,
	pub position: OffsetBeatRange,
}

impl MidiClip {
	pub fn diff(
		&self,
		state: &State,
		audio: &[f32],
		events: &mut Vec<Event>,
		voice_alloc: &mut VoiceAlloc<VoiceId, MidiNote>,
	) {
		debug_assert!(state.transport.playing);

		let position = state.transport.position.to_samples(&state.transport);

		let (start, end) = self.position.beat_range().to_samples(&state.transport);
		if !(start < position + audio.len() && end >= position) {
			return;
		}

		state.midi_patterns[&self.pattern]
			.notes
			.values()
			.filter_map(|&(mut note)| {
				note.position = (note.position + self.position.start())
					.saturating_sub(self.position.offset())?
					.clamp(self.position.beat_range())?;
				Some(note)
			})
			.for_each(|note| {
				let (start, end) = note.position.to_samples(&state.transport);
				if start < position
					&& end > position
					&& !voice_alloc.activate((self.id, note.id, note.key))
				{
					alloc_or_steal(events, voice_alloc, (self.id, note.id, note.key), note, 0);
				}
			});
	}

	pub fn process(
		&self,
		state: &State,
		audio: &[f32],
		events: &mut Vec<Event>,
		voice_alloc: &mut VoiceAlloc<VoiceId, MidiNote>,
	) {
		debug_assert!(state.transport.playing);

		let position = state.transport.position.to_samples(&state.transport);

		let (start, end) = self.position.beat_range().to_samples(&state.transport);
		if !(start < position + audio.len() && end >= position) {
			return;
		}

		state.midi_patterns[&self.pattern]
			.notes
			.values()
			.filter_map(|&(mut note)| {
				note.position = (note.position + self.position.start())
					.saturating_sub(self.position.offset())?
					.clamp(self.position.beat_range())?;
				Some(note)
			})
			.for_each(|note| {
				let (start, end) = note.position.to_samples(&state.transport);

				if let Some(time) = start.checked_sub(position)
					&& time < audio.len()
				{
					alloc_or_steal(
						events,
						voice_alloc,
						(self.id, note.id, note.key),
						note,
						time,
					);
				}

				if let Some(time) = end.checked_sub(position)
					&& time < audio.len()
				{
					dealloc(events, voice_alloc, (self.id, note.id, note.key), time);
				}
			});
	}
}

fn alloc_or_steal(
	events: &mut Vec<Event>,
	voice_alloc: &mut VoiceAlloc<VoiceId, MidiNote>,
	id: (MidiClipId, MidiNoteId, MidiKey),
	info: MidiNote,
	time: usize,
) {
	let voice = voice_alloc.alloc(id, info).unwrap_or_else(|| {
		let (voice, old_voice) = voice_alloc.steal(id, info, |l, r| {
			r.info
				.position
				.start()
				.cmp(&r.info.position.start())
				.then_with(|| l.info.velocity.total_cmp(&r.info.velocity))
				.then_with(|| l.info.key.cmp(&r.info.key).reverse())
		});

		events.push(Event::Off {
			time: time as u32 / 2,
			key: old_voice.info.key.0,
			velocity: old_voice.info.velocity,
			note_id: Match::Specific(old_voice.note_id),
		});

		voice
	});

	events.push(Event::On {
		time: time as u32 / 2,
		key: voice.info.key.0,
		velocity: voice.info.velocity,
		note_id: Match::Specific(voice.note_id),
	});
}

fn dealloc(
	events: &mut Vec<Event>,
	voice_alloc: &mut VoiceAlloc<VoiceId, MidiNote>,
	id: (MidiClipId, MidiNoteId, MidiKey),
	time: usize,
) {
	if let Some(voice) = voice_alloc.dealloc(id) {
		events.push(Event::Off {
			time: time as u32 / 2,
			key: voice.info.key.0,
			velocity: voice.info.velocity,
			note_id: Match::Specific(voice.note_id),
		});
	}
}
