use crate::{AudioGraphNode, AudioGraphNodeImpl};
use ahash::{AHashMap, AHashSet};
use std::{cmp::Ordering, collections::hash_map::Entry, sync::Mutex};

#[derive(Debug, Default)]
pub struct AudioGraph(Mutex<AudioGraphInner>);

impl AudioGraph {
    pub fn root(&self) -> AudioGraphNode {
        self.0.lock().unwrap().root.clone()
    }
}

#[derive(Debug)]
struct AudioGraphInner {
    root: AudioGraphNode,
    g: AHashMap<AudioGraphNode, AHashSet<AudioGraphNode>>,
    l: Vec<AudioGraphNode>,
    dirty: bool,
}

impl Default for AudioGraphInner {
    fn default() -> Self {
        let root = AudioGraphNode::default();

        Self {
            root: root.clone(),
            g: AHashMap::from_iter([(root.clone(), AHashSet::default())]),
            l: vec![root],
            dirty: false,
        }
    }
}

impl AudioGraphNodeImpl for AudioGraph {
    fn fill_buf(&self, buf_start_sample: usize, buf: &mut [f32]) {
        let AudioGraphInner {
            root, g, l, dirty, ..
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
            for s in buf.iter_mut() {
                *s = 0.0;
            }

            for node in &g[node] {
                node.fill_buf(buf_start_sample, buf);
            }

            node.fill_buf(buf_start_sample, buf);
        }
    }
}

impl AudioGraph {
    #[must_use]
    /// for now it's the caller's responsibility to make sure the graph stays acyclic
    pub fn connect(&self, from: &AudioGraphNode, to: &AudioGraphNode) -> bool {
        let AudioGraphInner { root, g, dirty, .. } = &mut *self.0.lock().unwrap();
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
    pub fn disconnect(&self, from: &AudioGraphNode, to: &AudioGraphNode) -> bool {
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
    pub fn add(&self, node: AudioGraphNode) -> bool {
        let AudioGraphInner { g, l, dirty, .. } = &mut *self.0.lock().unwrap();

        if let Entry::Vacant(vacant) = g.entry(node.clone()) {
            vacant.insert(AHashSet::default());
            l.push(node);

            *dirty = true;
            true
        } else {
            false
        }
    }

    #[must_use]
    pub fn remove(&self, node: &AudioGraphNode) -> bool {
        let AudioGraphInner {
            root, g, l, dirty, ..
        } = &mut *self.0.lock().unwrap();
        debug_assert_ne!(root, node);

        if g.remove(node).is_some() {
            let idx = l.iter().position(|n| n == node).unwrap();
            l.swap_remove(idx);

            for e in g.values_mut() {
                e.remove(node);
            }

            *dirty = true;
            true
        } else {
            false
        }
    }
}
