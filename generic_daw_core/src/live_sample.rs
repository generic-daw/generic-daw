use audio_graph::AudioGraphNodeImpl;
use std::sync::{
    atomic::{AtomicIsize, Ordering::SeqCst},
    Arc,
};

#[derive(Debug)]
pub struct LiveSample {
    audio: Arc<[f32]>,
    idx: AtomicIsize,
}

impl AudioGraphNodeImpl for LiveSample {
    fn fill_buf(&self, _: usize, buf: &mut [f32]) {
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
}

impl LiveSample {
    #[must_use]
    pub fn new(audio: Arc<[f32]>, before: usize) -> Self {
        Self {
            audio,
            idx: AtomicIsize::new(-isize::try_from(before).unwrap()),
        }
    }

    #[must_use]
    pub fn over(&self) -> bool {
        self.idx.load(SeqCst) as usize > self.audio.len()
    }
}
