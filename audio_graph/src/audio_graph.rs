use crate::{AudioGraphNode, AudioGraphNodeImpl};
use ahash::{AHashMap, AHashSet};
use std::collections::hash_map::Entry;

#[derive(Debug, Default)]
pub struct AudioGraph {
    root: AudioGraphNode,
    inner: AHashMap<AudioGraphNode, AHashSet<AudioGraphNode>>,
}

impl AudioGraphNodeImpl for AudioGraph {
    fn fill_buf(&self, _buf_start_sample: usize, _buf: &mut [f32]) {}
}

impl AudioGraph {
    #[must_use]
    /// for now it's the caller's responsibility to make sure the graph stays acyclic
    pub fn connect(&mut self, from: &AudioGraphNode, to: &AudioGraphNode) -> bool {
        self.inner
            .get_mut(from)
            .is_some_and(|v| v.insert(to.clone()))
    }

    #[must_use]
    pub fn disconnect(&mut self, from: &AudioGraphNode, to: &AudioGraphNode) -> bool {
        self.inner.get_mut(from).is_some_and(|v| v.remove(to))
    }

    #[must_use]
    pub fn add(&mut self, node: AudioGraphNode) -> bool {
        match self.inner.entry(node) {
            Entry::Occupied(_) => false,
            Entry::Vacant(vacant) => {
                vacant.insert(AHashSet::default());
                true
            }
        }
    }

    #[must_use]
    pub fn remove(&mut self, node: &AudioGraphNode) -> bool {
        if *node == self.root {
            return false;
        }

        if self.inner.remove(node).is_some() {
            for e in self.inner.values_mut() {
                e.remove(node);
            }

            true
        } else {
            false
        }
    }
}
