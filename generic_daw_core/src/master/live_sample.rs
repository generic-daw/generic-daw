use clap_host::Event;
use std::sync::{
    Arc,
    atomic::{
        AtomicIsize,
        Ordering::{AcqRel, Acquire},
    },
};

#[derive(Debug)]
pub struct LiveSample {
    audio: Arc<[f32]>,
    idx: AtomicIsize,
}

impl LiveSample {
    #[must_use]
    pub fn new(audio: Arc<[f32]>, before: usize) -> Self {
        Self {
            audio,
            idx: AtomicIsize::new(-(before as isize)),
        }
    }

    pub fn process(&self, audio: &mut [f32], _: &mut Vec<Event>) {
        let idx = self.idx.fetch_add(audio.len() as isize, AcqRel);

        let uidx = idx.unsigned_abs();

        if idx > 0 {
            self.audio[uidx..]
                .iter()
                .zip(audio)
                .for_each(|(s, buf)| *buf += s);
        } else {
            if uidx >= audio.len() {
                return;
            }

            self.audio
                .iter()
                .zip(audio[uidx..].iter_mut())
                .for_each(|(s, buf)| {
                    *buf += s;
                });
        }
    }

    pub fn over(&self) -> bool {
        self.idx
            .load(Acquire)
            .try_into()
            .is_ok_and(|idx: usize| idx >= self.audio.len())
    }
}
