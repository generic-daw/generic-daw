use crate::AudioGraphNodeImpl;

#[derive(Debug)]
pub struct AudioGraphNode(Box<dyn AudioGraphNodeImpl>);

impl AudioGraphNode {
    pub fn fill_buf(&self, buf_start_sample: usize, buf: &mut [f32]) {
        self.0.fill_buf(buf_start_sample, buf);
    }

    #[must_use]
    pub fn id(&self) -> crate::NodeId {
        self.0.id()
    }
}

impl<T> From<T> for AudioGraphNode
where
    T: AudioGraphNodeImpl + 'static,
{
    fn from(value: T) -> Self {
        Self(Box::new(value))
    }
}
