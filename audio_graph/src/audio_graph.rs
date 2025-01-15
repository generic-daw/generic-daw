use crate::{AudioGraphNode, AudioGraphNodeImpl};
use ahash::{AHashMap, AHashSet};
use std::{cmp::Ordering, collections::hash_map::Entry, sync::Mutex};

#[derive(Debug, Default)]
pub struct AudioGraph(Mutex<AudioGraphInner>);

#[derive(Debug, Default)]
struct AudioGraphInner {
    root: AudioGraphNode,
    g: AHashMap<AudioGraphNode, AHashSet<AudioGraphNode>>,
    l: Vec<AudioGraphNode>,
    b: Vec<f32>,
    dirty: bool,
}

impl AudioGraphNodeImpl for AudioGraph {
    fn fill_buf(&self, buf_start_sample: usize, buf: &mut [f32]) {
        let AudioGraphInner {
            root,
            g,
            l,
            b,
            dirty,
            ..
        } = &mut *self.0.lock().unwrap();

        if *dirty {
            *dirty = false;

            l.sort_unstable_by(|lhs, rhs| {
                if g[lhs].contains(rhs) {
                    Ordering::Less
                } else {
                    Ordering::Equal
                }
            });

            debug_assert_eq!(&l[0], root);
        }

        for node in l.iter().rev() {
            b.clear();
            b.resize(buf.len(), 0.0);

            for node in &g[node] {
                node.fill_buf(buf_start_sample, b);
            }

            node.fill_buf(buf_start_sample, b);
        }

        buf.iter_mut().zip(b).for_each(|(buf, b)| *buf = *b);
    }
}

impl AudioGraph {
    #[must_use]
    /// for now it's the caller's responsibility to make sure the graph stays acyclic
    pub fn connect(&mut self, from: &AudioGraphNode, to: &AudioGraphNode) -> bool {
        let AudioGraphInner { g, dirty, root, .. } = &mut *self.0.lock().unwrap();
        debug_assert_ne!(to, root);

        g.get_mut(from).is_some_and(|v| {
            if v.insert(to.clone()) {
                *dirty = true;
                true
            } else {
                false
            }
        })
    }

    #[must_use]
    pub fn disconnect(&mut self, from: &AudioGraphNode, to: &AudioGraphNode) -> bool {
        let AudioGraphInner { g, dirty, .. } = &mut *self.0.lock().unwrap();

        g.get_mut(from).is_some_and(|v| {
            if v.remove(to) {
                *dirty = true;
                true
            } else {
                false
            }
        })
    }

    #[expect(tail_expr_drop_order)]
    #[must_use]
    pub fn add(&mut self, node: AudioGraphNode) -> bool {
        let mut inner = self.0.lock().unwrap();

        if let Entry::Vacant(vacant) = inner.g.entry(node.clone()) {
            vacant.insert(AHashSet::default());
            inner.l.push(node);

            inner.dirty = true;

            true
        } else {
            false
        }
    }

    #[must_use]
    pub fn remove(&mut self, node: &AudioGraphNode) -> bool {
        let mut inner = self.0.lock().unwrap();

        if *node == inner.root {
            return false;
        }

        if inner.g.remove(node).is_some() {
            inner.dirty = true;

            let idx = inner.l.iter().position(|n| n == node).unwrap();
            inner.l.swap_remove(idx);

            for e in inner.g.values_mut() {
                e.remove(node);
            }

            drop(inner);

            true
        } else {
            false
        }
    }
}
