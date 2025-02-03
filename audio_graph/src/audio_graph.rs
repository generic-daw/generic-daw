use crate::{AudioGraphNode, AudioGraphNodeImpl};
use ahash::{AHashMap, AHashSet};
use std::{cmp::Ordering, sync::Mutex};

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
    c: AHashMap<AudioGraphNode, Vec<f32>>,
    dirty: bool,
}

impl Default for AudioGraphInner {
    fn default() -> Self {
        let root = AudioGraphNode::default();

        Self {
            root: root.clone(),
            g: AHashMap::from_iter([(root.clone(), AHashSet::default())]),
            l: vec![root.clone()],
            c: AHashMap::from_iter([(root, Vec::new())]),
            dirty: false,
        }
    }
}

impl AudioGraphNodeImpl for AudioGraph {
    fn fill_buf(&self, buf_start_sample: usize, buf: &mut [f32]) {
        let AudioGraphInner {
            root,
            g,
            l,
            c,
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
            for s in buf.iter_mut() {
                *s = 0.0;
            }

            for node in &g[node] {
                c[node]
                    .iter()
                    .zip(buf.iter_mut())
                    .for_each(|(sample, buf)| {
                        *buf += sample;
                    });
            }

            node.fill_buf(buf_start_sample, buf);

            let cbuf = c.get_mut(node).unwrap();
            cbuf.clear();
            cbuf.extend(&*buf);
        }
    }
}

impl AudioGraph {
    #[must_use]
    pub fn connect(&self, from: &AudioGraphNode, to: &AudioGraphNode) -> bool {
        let AudioGraphInner { g, dirty, .. } = &mut *self.0.lock().unwrap();

        if g.contains_key(to)
            && g.get(from).is_some_and(|g| !g.contains(to))
            && !Self::check_cycle(g, &mut AHashSet::with_capacity(g.len()), to, from)
        {
            g.get_mut(from).unwrap().insert(to.clone());
            *dirty = true;
            true
        } else {
            false
        }
    }

    #[must_use]
    pub fn disconnect(&self, from: &AudioGraphNode, to: &AudioGraphNode) -> bool {
        let AudioGraphInner { g, .. } = &mut *self.0.lock().unwrap();

        g.get_mut(from).is_some_and(|v| v.remove(to))
    }

    #[must_use]
    pub fn add(&self, node: &AudioGraphNode) -> bool {
        let AudioGraphInner { g, l, c, .. } = &mut *self.0.lock().unwrap();

        if g.contains_key(node) {
            false
        } else {
            g.insert(node.clone(), AHashSet::default());
            l.push(node.clone());
            c.insert(node.clone(), Vec::new());

            true
        }
    }

    #[must_use]
    pub fn remove(&self, node: &AudioGraphNode) -> bool {
        let AudioGraphInner { root, g, l, c, .. } = &mut *self.0.lock().unwrap();
        debug_assert_ne!(root, node);

        if g.remove(node).is_some() {
            let idx = l.iter().position(|n| n == node).unwrap();
            l.remove(idx);
            c.remove(node);

            for e in g.values_mut() {
                e.remove(node);
            }

            true
        } else {
            false
        }
    }

    fn check_cycle<'a>(
        g: &'a AHashMap<AudioGraphNode, AHashSet<AudioGraphNode>>,
        visited: &mut AHashSet<&'a AudioGraphNode>,
        current: &AudioGraphNode,
        to: &AudioGraphNode,
    ) -> bool {
        if current == to {
            return true;
        }

        for current in &g[current] {
            if visited.contains(current) {
                continue;
            }

            visited.insert(current);

            if Self::check_cycle(g, visited, current, to) {
                return true;
            }
        }

        false
    }
}
