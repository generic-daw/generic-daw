use std::sync::{
    atomic::{
        AtomicIsize,
        Ordering::{AcqRel, Acquire},
    },
    Arc,
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
            idx: AtomicIsize::new(before as isize),
        }
    }

    pub fn fill_buf(&self, _: usize, buf: &mut [f32]) {
        let idx = self.idx.fetch_add(buf.len() as isize, AcqRel);

        let uidx = idx.unsigned_abs();

        if idx > 0 {
            if uidx >= self.audio.len() {
                return;
            }

            self.audio[uidx..]
                .iter()
                .zip(buf)
                .for_each(|(s, buf)| *buf += s);
        } else {
            if uidx >= buf.len() {
                return;
            }

            self.audio
                .iter()
                .zip(buf[uidx..].iter_mut())
                .for_each(|(s, buf)| {
                    *buf += s;
                });
        }
    }

    pub fn over(&self) -> bool {
        self.idx
            .load(Acquire)
            .try_into()
            .is_ok_and(|idx: usize| idx < self.audio.len())
    }
}
