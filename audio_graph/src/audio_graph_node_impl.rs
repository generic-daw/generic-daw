use downcast_rs::{impl_downcast, DowncastSync};
use std::fmt::Debug;

pub trait AudioGraphNodeImpl: Debug + DowncastSync {
    /// If your node has any dependencies in the audio graph, this is expected to cache its output.
    ///
    /// The first time this is called, `buf` will contain the summed data from all dependencies.
    /// In any subsequent calls, don't rely on the contents of `buf`, rather just add the cached
    /// output to `buf`.
    fn fill_buf(&self, buf_start_sample: usize, buf: &mut [f32]);
}

impl_downcast!(sync AudioGraphNodeImpl);
