use crate::{node_id::NodeId, AudioGraphNode};
use ahash::{AHashMap, AHashSet};
use std::cmp::Ordering;

#[derive(Debug)]
pub struct AudioGraph {
    g: AHashMap<NodeId, AudioGraphEntry>,
    l: Vec<NodeId>,
    dirty: bool,
}

#[derive(Debug)]
struct AudioGraphEntry {
    pub node: AudioGraphNode,
    pub connections: AHashSet<NodeId>,
    pub cache: Vec<f32>,
}

impl AudioGraph {
    #[must_use]
    pub fn new(node: AudioGraphNode) -> Self {
        let id = node.id();
        let entry = AudioGraphEntry {
            node,
            connections: AHashSet::default(),
            cache: Vec::new(),
        };

        Self {
            g: AHashMap::from_iter([(id, entry)]),
            l: vec![id],
            dirty: false,
        }
    }

    pub fn fill_buf(&mut self, buf_start_sample: usize, buf: &mut [f32]) {
        if self.dirty {
            self.dirty = false;

            self.l.sort_unstable_by(|lhs, rhs| {
                if self.g[lhs].connections.contains(rhs) {
                    Ordering::Less
                } else {
                    Ordering::Equal
                }
            });
        }

        for node in self.l.iter().rev() {
            for s in buf.iter_mut() {
                *s = 0.0;
            }

            for node in &self.g[node].connections {
                self.g[node]
                    .cache
                    .iter()
                    .zip(buf.iter_mut())
                    .for_each(|(sample, buf)| {
                        *buf += sample;
                    });
            }

            self.g[node].node.fill_buf(buf_start_sample, buf);

            let cbuf = &mut self.g.get_mut(node).unwrap().cache;
            cbuf.clear();
            cbuf.extend(&*buf);
        }
    }

    #[must_use]
    pub fn connect(&mut self, from: NodeId, to: NodeId) -> bool {
        if self.g.contains_key(&to)
            && self
                .g
                .get(&from)
                .is_some_and(|g| !g.connections.contains(&to))
            && !self.check_cycle(&mut AHashSet::with_capacity(self.g.len()), to, from)
        {
            self.g.get_mut(&from).unwrap().connections.insert(to);
            self.dirty = true;
            true
        } else {
            false
        }
    }

    #[must_use]
    pub fn disconnect(&mut self, from: NodeId, to: NodeId) -> bool {
        self.g
            .get_mut(&from)
            .is_some_and(|g| g.connections.remove(&to))
    }

    #[must_use]
    pub fn add(&mut self, node: AudioGraphNode) -> bool {
        let id = node.id();

        if self.g.contains_key(&id) {
            false
        } else {
            let entry = AudioGraphEntry {
                node,
                connections: AHashSet::default(),
                cache: Vec::new(),
            };

            self.g.insert(id, entry);
            self.l.push(id);

            true
        }
    }

    #[must_use]
    pub fn remove(&mut self, node: NodeId) -> bool {
        debug_assert_ne!(self.l[0], node);

        if self.g.remove(&node).is_some() {
            self.g.remove(&node);
            self.l
                .remove(self.l.iter().position(|&n| n == node).unwrap());

            for g in self.g.values_mut() {
                g.connections.remove(&node);
            }

            true
        } else {
            false
        }
    }

    fn check_cycle(&self, visited: &mut AHashSet<NodeId>, current: NodeId, to: NodeId) -> bool {
        if current == to {
            return true;
        }

        for current in &self.g[&current].connections {
            if visited.contains(current) {
                continue;
            }

            visited.insert(*current);

            if self.check_cycle(visited, *current, to) {
                return true;
            }
        }

        false
    }
}
