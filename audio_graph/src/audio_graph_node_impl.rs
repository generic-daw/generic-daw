use std::fmt::Debug;

pub trait AudioGraphNodeImpl: Debug + Send + Sync {
    fn fill_buf(&self, buf_start_sample: usize, buf: &mut [f32]);
}
