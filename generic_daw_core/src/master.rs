use crate::{Action, Mixer, MusicalTime, daw_ctx::State, event::Event, resampler::Resampler};
use audio_graph::{NodeId, NodeImpl};
use generic_daw_utils::include_f32s;
use live_sample::LiveSample;
use std::sync::Arc;

mod live_sample;

static ON_BAR_CLICK: &[f32] = include_f32s!("../../assets/on_bar_click.pcm");
static OFF_BAR_CLICK: &[f32] = include_f32s!("../../assets/off_bar_click.pcm");

#[derive(Debug)]
pub struct Master {
	node: Mixer,

	click: Option<LiveSample>,
	on_bar_click: Arc<[f32]>,
	off_bar_click: Arc<[f32]>,
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
				let offset = (buf_end - buf_start).to_samples(&state.rtstate);

				let click = if buf_end
					.beat()
					.is_multiple_of(state.rtstate.numerator.into())
				{
					self.on_bar_click.clone()
				} else {
					self.off_bar_click.clone()
				};

				self.click = Some(LiveSample::new(click, offset));
			}
		}

		if let Some(c) = self.click.as_mut() {
			c.process(audio);

			if c.over() {
				self.click = None;
			}
		}

		self.node.process(state, audio, events);
	}

	fn id(&self) -> NodeId {
		self.node.id()
	}

	fn reset(&mut self) {
		self.click = None;
		self.node.reset();
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
			click: None,
			on_bar_click: on_bar_click.finish().into(),
			off_bar_click: off_bar_click.finish().into(),
			node: Mixer::default(),
		}
	}

	pub fn apply(&mut self, action: Action) {
		self.node.apply(action);
	}
}
