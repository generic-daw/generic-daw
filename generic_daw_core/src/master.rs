use crate::{Action, MixerNode, Position, Resampler, daw_ctx::State, event::Event};
use audio_graph::{NodeId, NodeImpl};
use generic_daw_utils::include_f32s;
use live_sample::LiveSample;
use std::sync::Arc;

mod live_sample;

static ON_BAR_CLICK: &[f32] = include_f32s!("../../assets/on_bar_click.pcm");
static OFF_BAR_CLICK: &[f32] = include_f32s!("../../assets/off_bar_click.pcm");

#[derive(Debug)]
pub struct Master {
    node: MixerNode,

    click: Option<LiveSample>,
    on_bar_click: Arc<[f32]>,
    off_bar_click: Arc<[f32]>,
}

impl NodeImpl for Master {
    type Event = Event;
    type State = State;

    fn process(&mut self, state: &Self::State, audio: &mut [f32], events: &mut Vec<Self::Event>) {
        if state.meter.playing && state.meter.metronome {
            let buf_start_pos = Position::from_samples(state.meter.sample, &state.meter);
            let mut buf_end_pos =
                Position::from_samples(state.meter.sample + audio.len(), &state.meter);

            if (buf_start_pos.beat() != buf_end_pos.beat() || buf_start_pos.step() == 0)
                && buf_end_pos.step() != 0
            {
                buf_end_pos = buf_end_pos.floor();
                let diff = (buf_end_pos - buf_start_pos).in_samples(&state.meter);

                let click = if buf_end_pos.beat() % u32::from(state.meter.numerator) == 0 {
                    self.on_bar_click.clone()
                } else {
                    self.off_bar_click.clone()
                };

                self.click = Some(LiveSample::new(click, diff));
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
    pub fn new(sample_rate: u32, node: MixerNode) -> Self {
        let mut on_bar_click = Resampler::new(44100, sample_rate as usize, 2).unwrap();
        on_bar_click.process(ON_BAR_CLICK);

        let mut off_bar_click = Resampler::new(44100, sample_rate as usize, 2).unwrap();
        off_bar_click.process(OFF_BAR_CLICK);

        Self {
            click: None,
            on_bar_click: on_bar_click.finish().into(),
            off_bar_click: off_bar_click.finish().into(),
            node,
        }
    }

    pub fn apply(&mut self, action: Action) {
        self.node.apply(action);
    }
}
