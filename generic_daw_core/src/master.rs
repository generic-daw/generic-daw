use crate::{Action, Mixer, MusicalTime, daw_ctx::State, event::Event, resampler::Resampler};
use audio_graph::{NodeId, NodeImpl};
use generic_daw_utils::include_f32s;
use std::sync::Arc;

static ON_BAR_CLICK: &[f32] = include_f32s!("../../assets/on_bar_click.pcm");
static OFF_BAR_CLICK: &[f32] = include_f32s!("../../assets/off_bar_click.pcm");

#[derive(Debug)]
pub struct Master {
	node: Mixer,

	on_bar_click: Arc<[f32]>,
	off_bar_click: Arc<[f32]>,
	click_on_bar: bool,
	click_sidx: isize,
}

impl NodeImpl for Master {
	type Event = Event;
	type State = State;

	fn process(
		&mut self,
		state: &mut Self::State,
		audio: &mut [f32],
		events: &mut Vec<Self::Event>,
	) {
		if state.rtstate.playing && state.rtstate.metronome {
			let buf_start = MusicalTime::from_samples(state.rtstate.sample, &state.rtstate);
			let mut buf_end =
				MusicalTime::from_samples(state.rtstate.sample + audio.len(), &state.rtstate);

			if (buf_start.beat() != buf_end.beat() || buf_start.tick() == 0) && buf_end.tick() != 0
			{
				buf_end = buf_end.floor();

				self.click_on_bar = buf_end
					.beat()
					.is_multiple_of(state.rtstate.numerator.into());

				self.click_sidx = -(buf_end - buf_start)
					.to_samples(&state.rtstate)
					.cast_signed();
			}
		}

		let click = if self.click_on_bar {
			&*self.on_bar_click
		} else {
			&*self.off_bar_click
		};

		let click_uidx = self.click_sidx.unsigned_abs();

		if click_uidx < click.len() && self.click_sidx >= 0 {
			click[click_uidx..]
				.iter()
				.zip(&mut *audio)
				.for_each(|(sample, buf)| *buf += sample);
		} else if click_uidx < audio.len() {
			click
				.iter()
				.zip(&mut audio[click_uidx..])
				.for_each(|(sample, buf)| *buf += sample);
		}

		self.click_sidx += audio.len().cast_signed();

		self.node.process(state, audio, events);
	}

	fn id(&self) -> NodeId {
		self.node.id()
	}

	fn reset(&mut self) {
		self.node.reset();
		self.click_sidx = isize::MIN;
	}

	fn delay(&self) -> usize {
		self.node.delay()
	}
}

impl Master {
	#[must_use]
	pub fn new(sample_rate: u32) -> Self {
		let mut on_bar_click = Resampler::new(44100, sample_rate as usize).unwrap();
		on_bar_click.process(ON_BAR_CLICK);

		let mut off_bar_click = Resampler::new(44100, sample_rate as usize).unwrap();
		off_bar_click.process(OFF_BAR_CLICK);

		Self {
			on_bar_click: on_bar_click.finish().into(),
			off_bar_click: off_bar_click.finish().into(),
			node: Mixer::default(),
			click_on_bar: false,
			click_sidx: isize::MIN,
		}
	}

	pub fn apply(&mut self, action: Action) {
		self.node.apply(action);
	}
}
