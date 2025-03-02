use crate::{AudioGraphNode, NodeId, audio_graph_entry::AudioGraphEntry};
use bit_set::BitSet;
use generic_daw_utils::HoleyVec;
use std::cmp::Ordering;

#[derive(Debug, Default)]
pub struct AudioGraph {
    graph: HoleyVec<AudioGraphEntry>,
    list: Vec<NodeId>,
    dirty: bool,
    visited: BitSet,
}

impl AudioGraph {
    #[must_use]
    pub fn new(node: AudioGraphNode) -> Self {
        let id = node.id();
        let entry = AudioGraphEntry {
            node,
            connections: BitSet::new(),
            cache: Vec::new(),
        };

        Self {
            graph: [Some(entry)].into(),
            list: vec![id],
            dirty: false,
            visited: BitSet::new(),
        }
    }

    #[must_use]
    pub fn root(&self) -> NodeId {
        self.list[0]
    }

    pub fn fill_buf(&mut self, buf: &mut [f32]) {
        if self.dirty {
            self.dirty = false;

            self.list.sort_unstable_by(|&lhs, &rhs| {
                if self.graph[*lhs].connections.contains(*rhs) {
                    Ordering::Less
                } else {
                    Ordering::Equal
                }
            });

            debug_assert_eq!(*self.list[0], 0);
        }

        for node in self.list.iter().copied().rev() {
            for s in &mut *buf {
                *s = 0.0;
            }

            for node in &self.graph[*node].connections {
                self.graph[node]
                    .cache
                    .iter()
                    .zip(&mut *buf)
                    .for_each(|(sample, buf)| {
                        *buf += sample;
                    });
            }

            self.graph[*node].node.fill_buf(buf);

            let cbuf = &mut self.graph.get_mut(*node).unwrap().cache;
            cbuf.clear();
            cbuf.extend(&*buf);
        }
    }

    pub fn reset(&self) {
        for entry in self.graph.values() {
            entry.node.reset();
        }
    }

    pub fn connect(&mut self, from: NodeId, to: NodeId) {
        if self.graph.get(*to).is_some()
            && self
                .graph
                .get(*from)
                .is_some_and(|entry| !entry.connections.contains(*to))
            && !Self::check_cycle(&self.graph, &mut self.visited, *to, *from)
        {
            self.visited.clear();
            self.graph.get_mut(*from).unwrap().connections.insert(*to);

            if !self.dirty {
                for id in self.list.iter().copied() {
                    if id == from {
                        break;
                    } else if id == to {
                        self.dirty = true;
                        break;
                    }
                }
            }
        }
    }

    pub fn disconnect(&mut self, from: NodeId, to: NodeId) {
        if let Some(entry) = self.graph.get_mut(*from) {
            entry.connections.remove(*to);
        }
    }

    pub fn insert(&mut self, node: AudioGraphNode) {
        let id = node.id();

        if let Some(entry) = self.graph.get_mut(*id) {
            entry.node = node;
            return;
        }

        let entry = AudioGraphEntry {
            node,
            connections: BitSet::new(),
            cache: Vec::new(),
        };

        self.graph.insert(*id, entry);
        self.list.push(id);
    }

    pub fn remove(&mut self, node: NodeId) {
        debug_assert_ne!(self.list[0], node);

        if self.graph.remove(*node).is_some() {
            let idx = self.list.iter().copied().position(|n| n == node).unwrap();
            self.list.remove(idx);

            for entry in self.graph.values_mut() {
                entry.connections.remove(*node);
            }
        }
    }

    fn check_cycle(
        graph: &HoleyVec<AudioGraphEntry>,
        visited: &mut BitSet,
        current: usize,
        to: usize,
    ) -> bool {
        current == to
            || graph[current].connections.iter().any(|current| {
                visited.insert(current) && Self::check_cycle(graph, visited, current, to)
            })
    }
}
