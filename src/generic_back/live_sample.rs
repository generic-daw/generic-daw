use std::{
    iter::repeat_n,
    sync::{
        atomic::{AtomicI32, Ordering::SeqCst},
        Arc,
    },
};

#[derive(Debug)]
pub struct LiveSample {
    sample: Arc<[f32]>,
    idx: AtomicI32,
}

impl LiveSample {
    pub fn new(sample: Arc<[f32]>, before: u32) -> Self {
        Self {
            sample,
            idx: AtomicI32::new(-i32::try_from(before).unwrap()),
        }
    }

    pub fn fill_buf(&self, buf: &mut [f32]) {
        let idx = self.idx.fetch_add(buf.len().try_into().unwrap(), SeqCst);

        if idx < 0 {
            repeat_n(0.0, usize::try_from(-idx).unwrap())
                .chain(self.sample.iter().copied())
                .zip(buf)
                .for_each(|(s, buf)| {
                    *buf += s;
                });
        } else {
            let idx = idx.try_into().unwrap();

            if idx >= self.sample.len() {
                return;
            }

            self.sample[idx..]
                .iter()
                .zip(buf)
                .for_each(|(s, buf)| *buf += s);
        }
    }

    pub fn over(&self) -> bool {
        usize::try_from(self.idx.load(SeqCst))
            .ok()
            .is_some_and(|idx| idx > self.sample.len())
    }
}
