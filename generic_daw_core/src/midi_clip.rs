use crate::{
	ClipId, Event, MidiKey, MidiNote, MidiNoteId, MidiPattern, MidiPatternId, Transport,
	audio_thread::{LoopSplit, State},
	time::OffsetBeatRange,
	voice_alloc::VoiceAlloc,
};
use clap_host::events::{EventFlags, Match};

pub type VoiceId = (ClipId, MidiNoteId, MidiKey);

#[derive(Clone, Copy, Debug)]
pub struct MidiClip {
	pub id: ClipId,
	pub pattern: MidiPatternId,
	pub position: OffsetBeatRange,
	pub stretch: f64,
}

impl MidiClip {
	#[must_use]
	pub fn new(pattern: MidiPatternId) -> Self {
		Self {
			id: ClipId::unique(),
			pattern,
			position: OffsetBeatRange::default(),
			stretch: 1.0,
		}
	}

	pub fn diff(
		&self,
		state: &State,
		events: &mut Vec<Event>,
		voice_alloc: &mut VoiceAlloc<VoiceId, MidiNote>,
	) {
		debug_assert!(state.transport.playing);

		let position = state.transport.position.to_frames(&state.transport);

		let (start, end) = self.position.beat_range().to_frames(&state.transport);
		if !(start < position && end >= position) {
			return;
		}

		state.midi_patterns[&self.pattern]
			.notes
			.values()
			.filter_map(|&(mut note)| {
				note.position = (note.position / self.stretch + self.position.start())
					.saturating_sub(self.position.offset())?
					.clamp(self.position.beat_range())?;
				Some(note)
			})
			.for_each(|note| {
				let (start, end) = note.position.to_frames(&state.transport);
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
		audio: &[[f32; 2]],
		events: &mut Vec<Event>,
		voice_alloc: &mut VoiceAlloc<VoiceId, MidiNote>,
	) {
		debug_assert!(state.transport.playing);

		let pattern = &state.midi_patterns[&self.pattern];
		state.transport.split_loop(audio.len(), |split, transport| {
			self.process_split(events, voice_alloc, transport, pattern, split);
		});
	}

	fn process_split(
		&self,
		events: &mut Vec<Event>,
		voice_alloc: &mut VoiceAlloc<VoiceId, MidiNote>,
		transport: &Transport,
		midi_pattern: &MidiPattern,
		split: &LoopSplit,
	) {
		let position = transport.position.to_frames(transport);

		let (start, end) = self.position.beat_range().to_frames(transport);
		if !(start < position + split.frames && end >= position) {
			return;
		}

		midi_pattern
			.notes
			.values()
			.filter_map(|&(mut note)| {
				note.position = (note.position / self.stretch + self.position.start())
					.saturating_sub(self.position.offset())?
					.clamp(self.position.beat_range())?;
				Some(note)
			})
			.for_each(|note| {
				let (start, end) = note.position.to_frames(transport);

				if let Some(time) = start.checked_sub(position)
					&& time < split.frames
				{
					alloc_or_steal(
						events,
						voice_alloc,
						(self.id, note.id, note.key),
						note,
						time + split.offset,
					);
				}

				if let Some(time) = end.checked_sub(position)
					&& time < split.frames
				{
					dealloc(
						events,
						voice_alloc,
						(self.id, note.id, note.key),
						time + split.offset,
					);
				}
			});
	}
}

fn alloc_or_steal(
	events: &mut Vec<Event>,
	voice_alloc: &mut VoiceAlloc<VoiceId, MidiNote>,
	id: VoiceId,
	info: MidiNote,
	time: usize,
) {
	let voice = voice_alloc.alloc(id, info).unwrap_or_else(|| {
		let (voice, old_voice) = voice_alloc.steal(id, info, |l, r| {
			l.info
				.position
				.start()
				.cmp(&r.info.position.start())
				.then_with(|| l.info.velocity.total_cmp(&r.info.velocity))
				.then_with(|| l.info.key.cmp(&r.info.key).reverse())
		});

		events.push(Event::Off {
			time: time as u32,
			key: old_voice.info.key.0,
			velocity: old_voice.info.velocity,
			note_id: Match::Specific(old_voice.note_id),
			flags: EventFlags::empty(),
		});

		voice
	});

	events.push(Event::On {
		time: time as u32,
		key: voice.info.key.0,
		velocity: voice.info.velocity,
		note_id: Match::Specific(voice.note_id),
		flags: EventFlags::empty(),
	});
}

fn dealloc(
	events: &mut Vec<Event>,
	voice_alloc: &mut VoiceAlloc<VoiceId, MidiNote>,
	id: VoiceId,
	time: usize,
) {
	if let Some(voice) = voice_alloc.dealloc(id) {
		events.push(Event::Off {
			time: time as u32,
			key: voice.info.key.0,
			velocity: voice.info.velocity,
			note_id: Match::Specific(voice.note_id),
			flags: EventFlags::empty(),
		});
	}
}
