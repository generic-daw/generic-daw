use crate::{Meter, MixerNode, Position, event::Event, resample_interleaved};
use audio_graph::{NodeId, NodeImpl};
use generic_daw_utils::include_f32s;
use live_sample::LiveSample;
use std::{
    cell::RefCell,
    sync::{Arc, atomic::Ordering::Acquire},
};

mod live_sample;

static ON_BAR_CLICK: &[f32] = include_f32s!("../../assets/on_bar_click.pcm");
static OFF_BAR_CLICK: &[f32] = include_f32s!("../../assets/off_bar_click.pcm");

#[derive(Debug)]
pub struct Master {
    /// information relating to the playback of the arrangement
    meter: Arc<Meter>,
    /// volume and pan
    node: Arc<MixerNode>,

    click: RefCell<Option<LiveSample>>,
    on_bar_click: Arc<[f32]>,
    off_bar_click: Arc<[f32]>,
}

impl NodeImpl<Event> for Master {
    fn process(&self, audio: &mut [f32], events: &mut Vec<Event>) {
        if self.meter.playing.load(Acquire) && self.meter.metronome.load(Acquire) {
            let sample = self.meter.sample.load(Acquire);
            let bpm = self.meter.bpm.load(Acquire);

            let buf_start_pos = Position::from_samples(sample, bpm, self.meter.sample_rate);
            let mut buf_end_pos =
                Position::from_samples(sample + audio.len(), bpm, self.meter.sample_rate);

            if (buf_start_pos.beat() != buf_end_pos.beat() && buf_end_pos.step() != 0)
                || buf_start_pos.step() == 0
            {
                buf_end_pos = buf_end_pos.floor();

                let diff = (buf_end_pos - buf_start_pos).in_samples(bpm, self.meter.sample_rate);
                let click = if buf_end_pos.beat() % self.meter.numerator.load(Acquire) as u32 == 0 {
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
            c.process(audio);

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
    pub fn new(meter: Arc<Meter>, node: Arc<MixerNode>) -> Self {
        let sample_rate = meter.sample_rate;

        Self {
            click: RefCell::default(),
            on_bar_click: resample_interleaved(44100, sample_rate, ON_BAR_CLICK.into())
                .unwrap()
                .into(),
            off_bar_click: resample_interleaved(44100, sample_rate, OFF_BAR_CLICK.into())
                .unwrap()
                .into(),
            meter,
            node,
        }
    }
}
