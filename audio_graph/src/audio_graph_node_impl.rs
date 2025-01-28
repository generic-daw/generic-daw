use std::fmt::Debug;

pub trait AudioGraphNodeImpl: Debug + Send + Sync {
    /// `buf` contains the summed data from all dependencies in the graph
    fn fill_buf(&self, buf_start_sample: usize, buf: &mut [f32]);
}
