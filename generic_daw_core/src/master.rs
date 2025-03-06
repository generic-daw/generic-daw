use crate::{Meter, MixerNode, Position, include_f32s, resample};
use audio_graph::{AudioGraphNodeImpl, NodeId};
use live_sample::LiveSample;
use std::{
    cell::RefCell,
    sync::{Arc, atomic::Ordering::Acquire},
};

mod include_f32s;
mod live_sample;

static ON_BAR_CLICK: &[f32] = include_f32s!("../../assets/on_bar_click.pcm");
static OFF_BAR_CLICK: &[f32] = include_f32s!("../../assets/off_bar_click.pcm");

#[derive(Debug)]
pub struct Master {
    /// information relating to the playback of the arrangement
    pub meter: Arc<Meter>,
    /// volume and pan
    pub node: Arc<MixerNode>,

    click: RefCell<Option<LiveSample>>,
    on_bar_click: Arc<[f32]>,
    off_bar_click: Arc<[f32]>,
}

impl AudioGraphNodeImpl for Master {
    fn fill_buf(&self, buf: &mut [f32]) {
        if self.meter.playing.load(Acquire) && self.meter.metronome.load(Acquire) {
            let sample = self.meter.sample.load(Acquire);
            let bpm = self.meter.bpm.load(Acquire);

            let buf_start_pos =
                Position::from_interleaved_samples(sample, bpm, self.meter.sample_rate);
            let mut buf_end_pos =
                Position::from_interleaved_samples(sample + buf.len(), bpm, self.meter.sample_rate);

            if (buf_start_pos.beat() != buf_end_pos.beat() && buf_end_pos.step() != 0)
                || buf_start_pos.step() == 0
            {
                buf_end_pos = buf_end_pos.floor();

                let diff = (buf_end_pos - buf_start_pos)
                    .in_interleaved_samples(bpm, self.meter.sample_rate);
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
            c.fill_buf(buf);

            if c.over() {
                *click = None;
            }
        }

        self.node.fill_buf(buf);
    }

    fn id(&self) -> NodeId {
        self.node.id()
    }

    fn reset(&self) {
        *self.click.borrow_mut() = None;
    }
}

impl Master {
    pub fn new(meter: Arc<Meter>) -> Self {
        let sample_rate = meter.sample_rate;

        Self {
            click: RefCell::default(),
            on_bar_click: resample(44100, sample_rate, ON_BAR_CLICK.into())
                .unwrap()
                .into(),
            off_bar_click: resample(44100, sample_rate, OFF_BAR_CLICK.into())
                .unwrap()
                .into(),
            meter,
            node: Arc::new(MixerNode::default()),
        }
    }
}
