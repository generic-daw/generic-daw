use crate::{resample, LiveSample, Meter, Position};
use arraydeque::{ArrayDeque, Wrapping};
use audio_graph::{AudioGraphNodeImpl, NodeId};
use include_data::include_f32s;
use std::{
    cell::RefCell,
    sync::{atomic::Ordering::Acquire, Arc},
};

static ON_BAR_CLICK: &[f32] = include_f32s!("../../assets/on_bar_click.pcm");
static OFF_BAR_CLICK: &[f32] = include_f32s!("../../assets/off_bar_click.pcm");

#[derive(Debug)]
pub struct Master {
    id: NodeId,
    pub meter: Arc<Meter>,
    pub live_sample_playback: RefCell<ArrayDeque<LiveSample, 2, Wrapping>>,
    pub on_bar_click: Arc<[f32]>,
    pub off_bar_click: Arc<[f32]>,
}

impl AudioGraphNodeImpl for Master {
    fn fill_buf(&self, buf_start_sample: usize, buf: &mut [f32]) {
        if self.meter.playing.load(Acquire) && self.meter.metronome.load(Acquire) {
            let buf_start_pos = Position::from_interleaved_samples(buf_start_sample, &self.meter);
            let mut buf_end_pos =
                Position::from_interleaved_samples(buf_start_sample + buf.len(), &self.meter);

            if (buf_start_pos.quarter_note() != buf_end_pos.quarter_note()
                && buf_end_pos.sub_quarter_note() != 0)
                || buf_start_pos.sub_quarter_note() == 0
            {
                buf_end_pos = buf_end_pos.floor();

                let diff = (buf_end_pos - buf_start_pos).in_interleaved_samples(&self.meter);
                let click = if buf_end_pos.quarter_note()
                    % self.meter.numerator.load(Acquire) as u32
                    == 0
                {
                    self.on_bar_click.clone()
                } else {
                    self.off_bar_click.clone()
                };

                let click = LiveSample::new(click, diff);

                self.live_sample_playback.borrow_mut().push_back(click);
            }
        }

        self.live_sample_playback.borrow().iter().for_each(|s| {
            s.fill_buf(buf_start_sample, buf);
        });
    }

    fn id(&self) -> NodeId {
        self.id
    }
}

impl Master {
    pub(crate) fn new(sample_rate: u32) -> Self {
        Self {
            id: NodeId::unique(),
            meter: Arc::new(Meter::new(sample_rate)),
            live_sample_playback: RefCell::default(),
            on_bar_click: resample(44100, sample_rate, ON_BAR_CLICK.into())
                .unwrap()
                .into(),
            off_bar_click: resample(44100, sample_rate, OFF_BAR_CLICK.into())
                .unwrap()
                .into(),
        }
    }
}
