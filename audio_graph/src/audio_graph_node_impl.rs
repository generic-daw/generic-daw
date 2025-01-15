use downcast_rs::{impl_downcast, Downcast, DowncastSend, DowncastSync};
use std::fmt::Debug;

pub trait AudioGraphNodeImpl:
    'static + Debug + Send + Sync + Downcast + DowncastSend + DowncastSync
{
    /// for the first call with a certain `buf_start_sample`, process `buf` as necessary
    ///
    /// for all directly consecutive calls with the same `buf_start_sample`, add the outputs from the first call
    fn fill_buf(&self, buf_start_sample: usize, buf: &mut [f32]);
}

impl_downcast!(sync AudioGraphNodeImpl);
