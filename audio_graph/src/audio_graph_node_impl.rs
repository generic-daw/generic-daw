use std::fmt::Debug;

pub trait AudioGraphNodeImpl: Debug + Send + Sync {
    /// for the first call with a certain `buf_start_sample`, process `buf` as necessary
    ///
    /// for all directly consecutive calls with the same `buf_start_sample`, add the outputs from the first call
    fn fill_buf(&self, buf_start_sample: usize, buf: &mut [f32]);
}
