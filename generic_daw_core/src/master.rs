use crate::{METER, MixerNode, Position, event::Event, include_f32s, resample_interleaved};
use audio_graph::{NodeId, NodeImpl};
use live_sample::LiveSample;
use std::{cell::RefCell, sync::Arc};

mod include_f32s;
mod live_sample;

static ON_BAR_CLICK: &[f32] = include_f32s!("../../assets/on_bar_click.pcm");
static OFF_BAR_CLICK: &[f32] = include_f32s!("../../assets/off_bar_click.pcm");

#[derive(Debug)]
pub struct Master {
    /// volume and pan
    node: Arc<MixerNode>,

    click: RefCell<Option<LiveSample>>,
    on_bar_click: Arc<[f32]>,
    off_bar_click: Arc<[f32]>,
}

impl NodeImpl<Event> for Master {
    fn process(&self, audio: &mut [f32], events: &mut Vec<Event>) {
        let meter = METER.load();

        if meter.playing && meter.metronome {
            let buf_start_pos = Position::from_samples(meter.sample, meter.bpm, meter.sample_rate);
            let mut buf_end_pos =
                Position::from_samples(meter.sample + audio.len(), meter.bpm, meter.sample_rate);

            if (buf_start_pos.beat() != buf_end_pos.beat() && buf_end_pos.step() != 0)
                || buf_start_pos.step() == 0
            {
                buf_end_pos = buf_end_pos.floor();

                let diff = (buf_end_pos - buf_start_pos).in_samples(meter.bpm, meter.sample_rate);
                let click = if buf_end_pos.beat() % meter.numerator as u32 == 0 {
                    self.on_bar_click.clone()
                } else {
                    self.off_bar_click.clone()
                };

                self.click
                    .borrow_mut()
                    .replace(LiveSample::new(click, diff));
            }
        }

        let mut click = self.click.borrow_mut();
        if let Some(c) = click.as_ref() {
            c.process(audio, events);

            if c.over() {
                *click = None;
            }
        }

        self.node.process(audio, events);
    }

    fn id(&self) -> NodeId {
        self.node.id()
    }

    fn reset(&self) {
        *self.click.borrow_mut() = None;
        self.node.reset();
    }

    fn delay(&self) -> usize {
        self.node.delay()
    }
}

impl Master {
    pub fn new(node: Arc<MixerNode>) -> Self {
        let sample_rate = METER.load().sample_rate;

        Self {
            click: RefCell::default(),
            on_bar_click: resample_interleaved(44100, sample_rate, ON_BAR_CLICK.into())
                .unwrap()
                .into(),
            off_bar_click: resample_interleaved(44100, sample_rate, OFF_BAR_CLICK.into())
                .unwrap()
                .into(),
            node,
        }
    }
}
