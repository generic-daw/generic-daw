use crate::{audio_graph_entry::AudioGraphEntry, AudioGraphNode, NodeId};
use bit_set::BitSet;
use std::cmp::Ordering;

#[derive(Debug, Default)]
pub struct AudioGraph {
    graph: Vec<Option<AudioGraphEntry>>,
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
            graph: vec![Some(entry)],
            list: vec![id],
            dirty: false,
            visited: BitSet::new(),
        }
    }

    #[must_use]
    pub fn root(&self) -> NodeId {
        self.list[0]
    }

    pub fn fill_buf(&mut self, buf_start_sample: usize, buf: &mut [f32]) {
        if self.dirty {
            self.dirty = false;

            self.list.sort_unstable_by(|&lhs, &rhs| {
                if self.graph[*lhs]
                    .as_ref()
                    .unwrap()
                    .connections
                    .contains(*rhs)
                {
                    Ordering::Less
                } else {
                    Ordering::Equal
                }
            });

            debug_assert_eq!(*self.list[0], 0);
        }

        for node in self.list.iter().copied().rev() {
            for s in buf.iter_mut() {
                *s = 0.0;
            }

            let entry = self.graph[*node].as_ref().unwrap();

            for node in &entry.connections {
                self.graph[node]
                    .as_ref()
                    .unwrap()
                    .cache
                    .iter()
                    .zip(&mut *buf)
                    .for_each(|(sample, buf)| {
                        *buf += sample;
                    });
            }

            entry.node.fill_buf(buf_start_sample, buf);

            let cbuf = &mut self.graph[*node].as_mut().unwrap().cache;
            cbuf.clear();
            cbuf.extend(&*buf);
        }
    }

    pub fn connect(&mut self, from: NodeId, to: NodeId) {
        if self.graph.get(*to).is_some_and(Option::is_some)
            && self
                .graph
                .get(*from)
                .and_then(Option::as_ref)
                .is_some_and(|entry| !entry.connections.contains(*to))
            && !Self::check_cycle(&self.graph, &mut self.visited, *to, *from)
        {
            self.graph[*from].as_mut().unwrap().connections.insert(*to);
            self.dirty = true;
            self.visited.clear();
        }
    }

    pub fn disconnect(&mut self, from: NodeId, to: NodeId) {
        if let Some(Some(entry)) = self.graph.get_mut(*from) {
            entry.connections.remove(*to);
        }
    }

    pub fn insert(&mut self, node: AudioGraphNode) {
        let id = node.id();

        if let Some(Some(entry)) = self.graph.get_mut(*id) {
            entry.node = node;
            return;
        } else if *id >= self.graph.len() {
            self.graph.resize_with(*id + 1, || None);
        }

        let entry = AudioGraphEntry {
            node,
            connections: BitSet::new(),
            cache: self.graph[0].as_ref().unwrap().cache.clone(),
        };

        self.graph[*id].replace(entry);
        self.list.push(id);
    }

    pub fn remove(&mut self, node: NodeId) {
        debug_assert_ne!(self.list[0], node);

        if self
            .graph
            .get_mut(*node)
            .is_some_and(|g| g.take().is_some())
        {
            let idx = self.list.iter().copied().position(|n| n == node).unwrap();
            self.list.remove(idx);

            for entry in self.graph.iter_mut().flatten() {
                entry.connections.remove(*node);
            }
        }
    }

    fn check_cycle(
        graph: &[Option<AudioGraphEntry>],
        visited: &mut BitSet,
        current: usize,
        to: usize,
    ) -> bool {
        current == to
            || graph[current]
                .as_ref()
                .unwrap()
                .connections
                .iter()
                .any(|current| {
                    visited.insert(current) && Self::check_cycle(graph, visited, current, to)
                })
    }
}
