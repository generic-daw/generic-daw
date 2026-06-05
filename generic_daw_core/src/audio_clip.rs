use crate::{ClipId, SampleId, Transition, audio_thread::State, time::OffsetBeatSpan};
use dsp::resample_cubic;

#[derive(Clone, Copy, Debug)]
pub struct AudioClip {
	pub id: ClipId,
	pub sample: SampleId,
	pub position: OffsetBeatSpan,
	pub volume: f32,
	pub fade_start: Transition,
	pub fade_end: Transition,
	pub stretch: f64,
}

impl AudioClip {
	#[must_use]
	pub fn new(sample: SampleId) -> Self {
		Self {
			id: ClipId::unique(),
			sample,
			position: OffsetBeatSpan::default(),
			volume: 1.0,
			fade_start: Transition::default(),
			fade_end: Transition::default(),
			stretch: 1.0,
		}
	}

	pub fn process(&self, state: &State, audio: &mut [f32]) {
		debug_assert!(state.transport.playing);

		let position = state.transport.position.to_samples(&state.transport);

		let start = self.position.start().to_samples(&state.transport);

		let write_start = start.saturating_sub(position);
		if write_start >= audio.len() {
			return;
		}

		let sample = &state.samples[&self.sample];

		let read_len = (sample.len(&state.transport))
			.saturating_sub(self.position.offset())
			.min(self.position.len() * self.stretch.abs());

		let len = (read_len / self.stretch.abs()).to_samples(&state.transport);

		let play_pos = position.saturating_sub(start);
		if play_pos >= len {
			return;
		}

		let resample_ratio = sample.resample_ratio(&state.transport);
		let offset = (self.position.offset() / resample_ratio).to_samples(&state.transport);
		let read_len = sample.samples.len() - offset;

		let resample_ratio = self.stretch / resample_ratio;
		let read_start = if resample_ratio.is_sign_positive() {
			offset
		} else {
			0
		};

		let fade_start = self.fade_start.len.to_samples(&state.transport);
		let fade_end = self.fade_end.len.to_samples(&state.transport);

		let mut iter = resample_cubic(
			&sample.samples[read_start..][..read_len],
			resample_ratio,
			play_pos / 2,
		)
		.take((len - play_pos) / 2)
		.zip(audio[write_start..].as_chunks_mut::<2>().0)
		.zip((play_pos..).step_by(2));

		iter.by_ref()
			.take(fade_start.saturating_sub(play_pos) / 2)
			.for_each(|(((l, r), buf), pos)| {
				let mix = self.fade_start.transition(pos as f32 / fade_start as f32);
				buf[0] += l * self.volume * mix;
				buf[1] += r * self.volume * mix;
			});

		iter.by_ref()
			.take((len - fade_end).saturating_sub(play_pos) / 2)
			.for_each(|(((l, r), buf), _)| {
				buf[0] += l * self.volume;
				buf[1] += r * self.volume;
			});

		iter.by_ref().for_each(|(((l, r), buf), pos)| {
			let mix = self
				.fade_end
				.transition((len - pos) as f32 / fade_end as f32);
			buf[0] += l * self.volume * mix;
			buf[1] += r * self.volume * mix;
		});
	}
}
