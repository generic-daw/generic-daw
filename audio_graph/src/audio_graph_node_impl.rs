use downcast_rs::{impl_downcast, DowncastSync};
use std::fmt::Debug;

pub trait AudioGraphNodeImpl: Debug + DowncastSync {
    /// `buf` contains the summed data from all dependencies
    fn fill_buf(&self, buf_start_sample: usize, buf: &mut [f32]);
}

impl_downcast!(sync AudioGraphNodeImpl);
