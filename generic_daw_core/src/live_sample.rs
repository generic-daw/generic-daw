use std::sync::{
    atomic::{AtomicIsize, Ordering::SeqCst},
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
            idx: AtomicIsize::new(-isize::try_from(before).unwrap()),
        }
    }

    pub fn fill_buf(&self, buf: &mut [f32]) {
        let idx = self
            .idx
            .fetch_add(isize::try_from(buf.len()).unwrap(), SeqCst);

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

    #[must_use]
    pub fn over(&self) -> bool {
        self.idx.load(SeqCst) as usize > self.audio.len()
    }
}
