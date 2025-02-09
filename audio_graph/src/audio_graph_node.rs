use crate::AudioGraphNodeImpl;
use std::ops::Deref;

#[derive(Debug)]
pub struct AudioGraphNode(Box<dyn AudioGraphNodeImpl>);

impl Deref for AudioGraphNode {
    type Target = dyn AudioGraphNodeImpl;

    fn deref(&self) -> &Self::Target {
        &*self.0
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
