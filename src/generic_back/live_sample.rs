use std::sync::{
    atomic::{AtomicI32, Ordering::SeqCst},
    Arc,
};

#[derive(Debug)]
pub struct LiveSample {
    audio: Arc<[f32]>,
    idx: AtomicI32,
}

impl LiveSample {
    pub fn new(audio: Arc<[f32]>, before: u32) -> Self {
        Self {
            audio,
            idx: AtomicI32::new(-i32::try_from(before).unwrap()),
        }
    }

    pub fn fill_buf(&self, buf: &mut [f32]) {
        let idx = self.idx.fetch_add(buf.len().try_into().unwrap(), SeqCst);

        if idx > 0 {
            let idx = idx.try_into().unwrap();

            if idx >= self.audio.len() {
                return;
            }

            self.audio[idx..]
                .iter()
                .zip(buf)
                .for_each(|(s, buf)| *buf += s);
        } else {
            let idx = (-idx).try_into().unwrap();

            if idx >= buf.len() {
                return;
            }

            self.audio
                .iter()
                .copied()
                .zip(buf[idx..].iter_mut())
                .for_each(|(s, buf)| {
                    *buf += s;
                });
        }
    }

    pub fn over(&self) -> bool {
        usize::try_from(self.idx.load(SeqCst))
            .ok()
            .is_some_and(|idx| idx > self.audio.len())
    }
}
