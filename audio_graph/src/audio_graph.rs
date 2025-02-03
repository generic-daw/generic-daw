use crate::{AudioGraphNode, AudioGraphNodeImpl as _};
use ahash::{AHashMap, AHashSet};
use std::cmp::Ordering;

#[derive(Debug)]
pub struct AudioGraph {
    root: AudioGraphNode,
    g: AHashMap<AudioGraphNode, AHashSet<AudioGraphNode>>,
    l: Vec<AudioGraphNode>,
    c: AHashMap<AudioGraphNode, Vec<f32>>,
    dirty: bool,
}

impl AudioGraph {
    #[must_use]
    pub fn new(root: AudioGraphNode) -> Self {
        Self {
            root: root.clone(),
            g: AHashMap::from_iter([(root.clone(), AHashSet::default())]),
            l: vec![root.clone()],
            c: AHashMap::from_iter([(root, Vec::new())]),
            dirty: false,
        }
    }

    pub fn fill_buf(&mut self, buf_start_sample: usize, buf: &mut [f32]) {
        if self.dirty {
            self.dirty = false;

            self.l.sort_unstable_by(|lhs, rhs| {
                if self.g[lhs].contains(rhs) {
                    Ordering::Less
                } else {
                    Ordering::Equal
                }
            });

            debug_assert_eq!(self.l[0], self.root);
        }

        for node in self.l.iter().rev() {
            for s in buf.iter_mut() {
                *s = 0.0;
            }

            for node in &self.g[node] {
                self.c[node]
                    .iter()
                    .zip(buf.iter_mut())
                    .for_each(|(sample, buf)| {
                        *buf += sample;
                    });
            }

            node.fill_buf(buf_start_sample, buf);

            let cbuf = self.c.get_mut(node).unwrap();
            cbuf.clear();
            cbuf.extend(&*buf);
        }
    }

    #[must_use]
    pub fn connect(&mut self, from: &AudioGraphNode, to: AudioGraphNode) -> bool {
        if self.g.contains_key(&to)
            && self.g.get(from).is_some_and(|g| !g.contains(&to))
            && !self.check_cycle(&mut AHashSet::with_capacity(self.g.len()), &to, from)
        {
            self.g.get_mut(from).unwrap().insert(to);
            self.dirty = true;
            true
        } else {
            false
        }
    }

    #[must_use]
    pub fn disconnect(&mut self, from: &AudioGraphNode, to: &AudioGraphNode) -> bool {
        self.g.get_mut(from).is_some_and(|v| v.remove(to))
    }

    #[must_use]
    pub fn add(&mut self, node: AudioGraphNode) -> bool {
        if self.g.contains_key(&node) {
            false
        } else {
            self.g.insert(node.clone(), AHashSet::default());
            self.l.push(node.clone());
            self.c
                .insert(node, Vec::with_capacity(self.c[&self.root].len()));

            true
        }
    }

    #[must_use]
    pub fn remove(&mut self, node: &AudioGraphNode) -> bool {
        debug_assert_ne!(&self.root, node);

        if self.g.remove(node).is_some() {
            let idx = self.l.iter().position(|n| n == node).unwrap();
            self.l.remove(idx);
            self.c.remove(node);

            for e in self.g.values_mut() {
                e.remove(node);
            }

            true
        } else {
            false
        }
    }

    fn check_cycle<'a>(
        &'a self,
        visited: &mut AHashSet<&'a AudioGraphNode>,
        current: &AudioGraphNode,
        to: &AudioGraphNode,
    ) -> bool {
        if current == to {
            return true;
        }

        for current in &self.g[current] {
            if visited.contains(current) {
                continue;
            }

            visited.insert(current);

            if self.check_cycle(visited, current, to) {
                return true;
            }
        }

        false
    }
}
